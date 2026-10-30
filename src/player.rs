//! The player, which contains all of the core logic behind the music player.

use crate::history::History;
use std::sync::Arc;
use tokio::sync::mpsc::{self, Receiver};

use crate::{
    audio::waiter,
    bookmark::Bookmarks,
    downloader,
    tracks::{self, List},
    ui,
    volume::PersistentVolume,
    Message, Tasks,
};

/// Represents the currently known playback state.
///
/// * [`Current::Loading`] indicates the player is waiting for data.
/// * [`Current::Track`] indicates the player has a decoded track available.
#[derive(Clone, Debug)]
pub enum Current {
    /// Waiting for a track to arrive. The optional `Progress` is used to
    /// indicate global download progress when present.
    Loading(Option<downloader::Progress>),

    /// A decoded track that can be played; contains the track `Info`.
    Track(tracks::Info),
}

impl Default for Current {
    fn default() -> Self {
        // By default the player starts in a loading state with no progress.
        Self::Loading(None)
    }
}

impl Current {
    /// Returns `true` if this `Current` value represents a loading state.
    pub const fn loading(&self) -> bool {
        matches!(self, Self::Loading(_))
    }
}

/// The high-level application player.
///
/// `Player` composes the downloader, UI, audio sink and bookkeeping state.
/// It owns background `Handle`s and drives the main message loop in `run`.
pub struct Player {
    /// Persistent bookmark storage used by the player.
    bookmarks: Bookmarks,
    history: History,
    generation: u64,

    /// Current playback state (loading or track).
    current: Current,

    /// Background downloader that fills the internal queue.
    downloader: downloader::Handle,

    /// Receiver for incoming `Message` commands.
    rx: Receiver<crate::Message>,

    /// Shared audio sink used for playback.
    sink: Arc<rodio::Player>,

    /// UI handle for rendering and input.
    ui: ui::Handle,

    /// Notifies when a play head has been appended.
    waiter: waiter::Handle,
}

impl Player {
    /// Sets the in-memory current state and notifies the UI about the change.
    ///
    /// If the new state is a `Track`, this will also update the bookmarked flag
    /// based on persistent bookmarks.
    pub fn set_current(&mut self, current: Current) -> crate::Result<()> {
        self.current = current.clone();
        self.ui.update(ui::Update::Track(current))?;

        let Current::Track(track) = &self.current else {
            return Ok(());
        };

        let bookmarked = self.bookmarks.bookmarked(track);
        self.ui.update(ui::Update::Bookmarked(bookmarked))?;

        Ok(())
    }

    /// Initialize a `Player` with the provided CLI `args` and audio `mixer`.
    ///
    /// This sets up the audio sink, UI, downloader, bookmarks and persistent
    /// volume state. The function returns a fully constructed `Player` ready
    /// to be driven via `run`.
    pub async fn init(
        settings: crate::config::Settings,
        bindings: crate::config::BindingMap,
        mixer: &rodio::mixer::Mixer,
    ) -> crate::Result<(Self, crate::Tasks)> {
        let (tx, rx) = mpsc::channel(8);
        let mut tasks = Tasks::new(tx.clone());
        if settings.playback.paused {
            tx.send(Message::Pause).await?;
        }

        tx.send(Message::Init).await?;
        let list = List::load(settings.playback.track_list.as_ref()).await?;

        let sink = Arc::new(rodio::Player::connect_new(mixer));
        let state = ui::State::initial(Arc::clone(&sink), list.name.clone());

        let volume = PersistentVolume::load().await?;
        sink.set_volume(volume.float());

        let ui = tasks.ui(state, &settings, bindings).await?;
        let player = Self {
            downloader: tasks.downloader(&settings, ui.logger(), list)?,
            ui,
            waiter: tasks.waiter(Arc::clone(&sink)),
            bookmarks: Bookmarks::load().await?,
            current: Current::default(),
            history: History::default(),
            generation: 0,
            rx,
            sink,
        };

        Ok((player, tasks))
    }

    /// Close any outlying processes, as well as persist state that
    /// should survive such as bookmarks and volume.
    #[cfg_attr(test, mutants::skip)]
    pub async fn close(self) -> crate::Result<()> {
        self.sink.stop();
        self.bookmarks.save().await?;
        PersistentVolume::save(self.sink.volume()).await?;

        Ok(())
    }

    fn can_go_previous(&self) -> bool {
        self.history.has_previous()
            || matches!(self.current, Current::Track(_))
                && History::should_restart(self.sink.get_pos())
            || self.current.loading() && self.history.current().is_some()
    }

    fn update_previous_capability(&mut self) -> crate::Result<()> {
        self.ui
            .update(ui::Update::CanGoPrevious(self.can_go_previous()))
    }

    fn play(&mut self, queued: tracks::Queued) -> crate::Result<()> {
        let decoded = queued.decode()?;
        let paused = self.sink.is_paused();
        // Rodio drains a stopped source through the mixer. Resume long enough for that
        // drain when changing tracks from a paused state, then restore the user's pause.
        if paused {
            self.sink.play();
        }
        self.sink.stop();
        self.sink.append(decoded.data);
        if paused {
            self.sink.pause();
        }
        self.set_current(Current::Track(decoded.info))?;
        self.generation = self.generation.wrapping_add(1);
        self.waiter.notify(self.generation);
        self.update_previous_capability()?;
        Ok(())
    }

    fn play_current_history(&mut self) -> crate::Result<bool> {
        let Some(track) = self.history.current().cloned() else {
            return Ok(false);
        };
        self.play(track)?;
        Ok(true)
    }

    fn advance(&mut self) -> crate::Result<()> {
        if let Some(track) = self.history.next().cloned() {
            return self.play(track);
        }
        match self.downloader.track() {
            downloader::Output::Loading(progress) => {
                self.generation = self.generation.wrapping_add(1);
                self.sink.stop();
                self.set_current(Current::Loading(progress))?;
                self.update_previous_capability()?;
            }
            downloader::Output::Queued(queued) => {
                self.history.push(queued);
                self.play_current_history()?;
            }
        }
        Ok(())
    }

    fn previous(&mut self) -> crate::Result<()> {
        if self.current.loading() {
            let _ = self.play_current_history()?;
        } else if History::should_restart(self.sink.get_pos()) {
            let _ = self.play_current_history()?;
            self.ui.update(ui::Update::Restarted)?;
        } else if let Some(track) = self.history.previous().cloned() {
            self.play(track)?;
        }
        self.update_previous_capability()
    }

    /// Applies one command and publishes its observable state updates.
    fn apply(&mut self, message: Message) -> crate::Result<bool> {
        match message {
            Message::Next if self.current.loading() => return Ok(false),
            Message::Next | Message::Init => self.advance()?,
            Message::Loaded if self.current.loading() => self.advance()?,
            Message::Loaded => return Ok(false),
            Message::Finished(generation) if generation == self.generation => self.advance()?,
            Message::Finished(_) => return Ok(false),
            Message::Previous => self.previous()?,
            Message::PreviousAvailable(generation) if generation == self.generation => {
                self.update_previous_capability()?
            }
            Message::PreviousAvailable(_) => return Ok(false),
            Message::Play => {
                self.sink.play();
            }
            Message::Pause => {
                self.sink.pause();
            }
            Message::PlayPause => {
                if self.sink.is_paused() {
                    self.sink.play();
                } else {
                    self.sink.pause();
                }
            }
            Message::ChangeVolume(change) => {
                self.sink
                    .set_volume((self.sink.volume() + change).clamp(0.0, 1.0));
                self.ui.update(ui::Update::Volume)?;
            }
            Message::SetVolume(volume) => {
                self.sink.set_volume(volume.clamp(0.0, 1.0));
                self.ui.update(ui::Update::Volume)?;
            }
            Message::Bookmark => {
                let Current::Track(current) = &self.current else {
                    return Ok(false);
                };
                let bookmarked = self.bookmarks.bookmark(current)?;
                self.ui.update(ui::Update::Bookmarked(bookmarked))?;
            }
            Message::Quit => return Ok(false),
        }
        Ok(true)
    }

    /// Drives the existing terminal and native-adapter command loop.
    pub async fn run(&mut self) -> crate::Result<()> {
        while let Some(message) = self.rx.recv().await {
            if message == Message::Quit {
                break;
            }
            if !self.apply(message)? {
                continue;
            }
            #[cfg(feature = "mpris")]
            self.ui.mpris.handle(&message).await?;
            #[cfg(all(feature = "media-controls", windows))]
            if let Some(controls) = self.ui.media_controls.as_mut() {
                controls.handle(&message)?;
            }
        }
        Ok(())
    }
}

#[cfg(all(test, not(feature = "mpris")))]
mod tests {
    use super::{Current, Player};
    use crate::ui::Update;
    use crate::{
        audio::waiter, bookmark::Bookmarks, downloader, tests::fixture, tracks::Queued, Message,
    };
    use rodio::queue::SourcesQueueOutput;
    use std::time::Duration;

    fn player() -> (Player, tokio::sync::broadcast::Receiver<Update>) {
        let (player, receiver, _) = player_with_tracks([]);
        (player, receiver)
    }

    fn player_with_tracks(
        tracks: impl IntoIterator<Item = Queued>,
    ) -> (
        Player,
        tokio::sync::broadcast::Receiver<Update>,
        SourcesQueueOutput,
    ) {
        let (ui, receiver) = crate::ui::Handle::for_test();
        let (_, rx) = tokio::sync::mpsc::channel(1);
        let (sink, output) = rodio::Player::new();
        (
            Player {
                bookmarks: Bookmarks {
                    entries: Vec::new(),
                },
                current: Default::default(),
                downloader: downloader::Handle::with_tracks(tracks),
                sink: std::sync::Arc::new(sink),
                ui,
                rx,
                waiter: waiter::Handle::idle(),
                history: Default::default(),
                generation: 0,
            },
            receiver,
            output,
        )
    }

    fn current_name(player: &Player) -> &str {
        match &player.current {
            Current::Track(track) => &track.display,
            Current::Loading(_) => panic!("expected a playing track"),
        }
    }

    fn render(output: &mut SourcesQueueOutput, samples: usize) {
        for sample in output.by_ref().take(samples) {
            assert!(sample.is_finite());
        }
    }

    fn handle_while_rendering(
        player: &mut Player,
        output: &mut SourcesQueueOutput,
        message: Message,
    ) -> bool {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let command = scope.spawn(|| sender.send(player.apply(message)).unwrap());
            for _ in 0..100_000 {
                if let Ok(outcome) = receiver.try_recv() {
                    command.join().unwrap();
                    return outcome.unwrap();
                }
                assert!(output.next().is_some_and(|sample| sample.is_finite()));
                std::thread::yield_now();
            }
            panic!("playback command did not finish while rendering offline");
        })
    }

    #[test]
    fn playback_commands_transition_valid_tracks_and_preserve_pause() {
        let tracks = [fixture::queued("one"), fixture::queued("two")];
        let (mut player, mut updates, mut output) = player_with_tracks(tracks);

        assert_eq!(player.apply(Message::Init).unwrap(), true);
        assert_eq!(current_name(&player), "one");
        assert_eq!(player.generation, 1);
        render(&mut output, 1_024);
        assert!(player.sink.get_pos() > Duration::ZERO);
        assert!(
            matches!(updates.try_recv(), Ok(Update::Track(Current::Track(track))) if track.display == "one")
        );
        assert!(matches!(updates.try_recv(), Ok(Update::Bookmarked(false))));
        assert!(matches!(
            updates.try_recv(),
            Ok(Update::CanGoPrevious(false))
        ));

        player.apply(Message::Pause).unwrap();
        assert!(player.sink.is_paused());
        assert_eq!(
            handle_while_rendering(&mut player, &mut output, Message::Next),
            true
        );
        assert_eq!(current_name(&player), "two");
        assert_eq!(player.generation, 2);
        assert!(player.sink.is_paused());
        assert!(
            matches!(updates.try_recv(), Ok(Update::Track(Current::Track(track))) if track.display == "two")
        );
        assert!(matches!(updates.try_recv(), Ok(Update::Bookmarked(false))));
        assert!(matches!(
            updates.try_recv(),
            Ok(Update::CanGoPrevious(true))
        ));

        player.apply(Message::Play).unwrap();
        assert!(!player.sink.is_paused());
        assert_eq!(
            handle_while_rendering(&mut player, &mut output, Message::Previous),
            true
        );
        assert_eq!(current_name(&player), "one");
        assert_eq!(player.generation, 3);
        assert!(
            matches!(updates.try_recv(), Ok(Update::Track(Current::Track(track))) if track.display == "one")
        );
        assert!(matches!(updates.try_recv(), Ok(Update::Bookmarked(false))));
        assert!(matches!(
            updates.try_recv(),
            Ok(Update::CanGoPrevious(false))
        ));

        assert_eq!(
            handle_while_rendering(&mut player, &mut output, Message::Next),
            true
        );
        assert_eq!(current_name(&player), "two");
        assert_eq!(player.generation, 4);
    }

    #[test]
    fn matching_generation_advances_and_stale_messages_preserve_transition_state() {
        let tracks = [fixture::queued("one"), fixture::queued("two")];
        let (mut player, mut updates, mut output) = player_with_tracks(tracks);
        player.apply(Message::Init).unwrap();
        render(&mut output, 1_024);
        while updates.try_recv().is_ok() {}
        let first_generation = player.generation;

        assert_eq!(
            handle_while_rendering(
                &mut player,
                &mut output,
                Message::Finished(first_generation)
            ),
            true
        );
        assert_eq!(current_name(&player), "two");
        let second_generation = player.generation;
        while updates.try_recv().is_ok() {}

        assert_eq!(
            player.apply(Message::Finished(first_generation)).unwrap(),
            false
        );
        assert_eq!(
            player
                .apply(Message::PreviousAvailable(first_generation))
                .unwrap(),
            false
        );
        assert_eq!(current_name(&player), "two");
        assert_eq!(player.generation, second_generation);
        assert!(updates.try_recv().is_err());

        assert_eq!(
            player
                .apply(Message::PreviousAvailable(second_generation))
                .unwrap(),
            true
        );
        assert!(matches!(
            updates.try_recv(),
            Ok(Update::CanGoPrevious(true))
        ));
    }

    #[test]
    fn loaded_is_ignored_after_a_track_has_started() {
        let (mut player, mut updates, _) = player_with_tracks([fixture::queued("one")]);
        player.apply(Message::Init).unwrap();
        while updates.try_recv().is_ok() {}

        assert_eq!(player.apply(Message::Loaded).unwrap(), false);
        assert_eq!(current_name(&player), "one");
        assert!(updates.try_recv().is_err());
    }

    #[test]
    fn previous_restarts_a_rendered_track_after_three_seconds() {
        let (mut player, mut updates, mut output) = player_with_tracks([fixture::queued("one")]);
        player.apply(Message::Init).unwrap();
        while updates.try_recv().is_ok() {}
        render(&mut output, 160_000);
        assert!(player.sink.get_pos() >= Duration::from_secs(3));
        assert!(player.can_go_previous());

        handle_while_rendering(&mut player, &mut output, Message::Previous);
        assert_eq!(current_name(&player), "one");
        assert_eq!(player.generation, 2);
        assert!(
            matches!(updates.try_recv(), Ok(Update::Track(Current::Track(track))) if track.display == "one")
        );
        assert!(matches!(updates.try_recv(), Ok(Update::Bookmarked(false))));
        assert!(matches!(updates.try_recv(), Ok(Update::CanGoPrevious(_))));
        assert!(matches!(updates.try_recv(), Ok(Update::Restarted)));
        assert!(matches!(updates.try_recv(), Ok(Update::CanGoPrevious(_))));
    }

    #[test]
    fn stale_generation_messages_are_ignored() {
        let (mut player, mut receiver) = player();
        assert_eq!(player.apply(Message::Finished(1)).unwrap(), false);
        assert_eq!(player.apply(Message::PreviousAvailable(1)).unwrap(), false);
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn loading_invalidates_completion_and_previous_restores_retained_audio() {
        let (mut player, mut updates, mut output) = player_with_tracks([fixture::queued("one")]);
        player.apply(Message::Init).unwrap();
        render(&mut output, 1_024);
        let generation = player.generation;
        handle_while_rendering(&mut player, &mut output, Message::Next);
        assert!(player.current.loading());
        assert_ne!(player.generation, generation);
        while updates.try_recv().is_ok() {}
        assert!(!player.apply(Message::Finished(generation)).unwrap());
        assert!(!player
            .apply(Message::PreviousAvailable(generation))
            .unwrap());
        assert!(updates.try_recv().is_err());
        handle_while_rendering(&mut player, &mut output, Message::Previous);
        assert_eq!(current_name(&player), "one");
        assert!(!player.apply(Message::Loaded).unwrap());
    }
}
