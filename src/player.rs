//! The playback core and its frontend-neutral state boundary.

use std::{sync::Arc, time::Duration};

use crate::history::History;
use tokio::sync::broadcast;

use crate::{
    audio::waiter, bookmark::Bookmarks, downloader, tracks, volume::PersistentVolume, Message,
};

/// Represents the currently known playback state.
#[derive(Clone, Debug)]
pub(crate) enum Current {
    /// Waiting for a track to arrive. The optional progress can be displayed by a frontend.
    Loading(Option<downloader::Progress>),
    /// A decoded track that can be played.
    Track(tracks::Info),
}

impl Default for Current {
    fn default() -> Self {
        Self::Loading(None)
    }
}

impl Current {
    /// Returns whether playback is waiting for a track.
    pub(crate) const fn loading(&self) -> bool {
        matches!(self, Self::Loading(_))
    }
}

/// Live, read-only audio state shared by frontends.
///
/// Cloning this view shares the player; it does not snapshot its values. Send
/// mutations through [`Message`] so the core can notify every frontend.
#[derive(Clone)]
pub(crate) struct PlaybackView(Arc<rodio::Player>);

impl PlaybackView {
    pub(crate) fn new(player: Arc<rodio::Player>) -> Self {
        Self(player)
    }

    pub(crate) fn volume(&self) -> f32 {
        self.0.volume()
    }
    pub(crate) fn is_paused(&self) -> bool {
        self.0.is_paused()
    }
    pub(crate) fn position(&self) -> Duration {
        self.0.get_pos()
    }
    #[allow(dead_code)]
    pub(crate) fn rate(&self) -> f32 {
        self.0.speed()
    }
}

/// Initial frontend state: copied track metadata and a live playback view.
///
/// Apply [`Update`]s to keep `current` and `bookmarked` in sync. Cloning this
/// state shares the playback view but does not subscribe to future updates.
#[derive(Clone)]
pub(crate) struct State {
    pub(crate) playback: PlaybackView,
    pub(crate) current: Current,
    pub(crate) bookmarked: bool,
    #[allow(dead_code)]
    pub(crate) track_list: String,
}

impl State {
    pub(crate) fn initial(playback: PlaybackView, track_list: String) -> Self {
        Self {
            playback,
            current: Current::default(),
            bookmarked: false,
            track_list,
        }
    }
}

/// Ordered changes published by the playback core.
///
/// Decoded-track updates precede their bookmark status. Volume, playback and rate events
/// tell observers to reread the corresponding live [`PlaybackView`] value.
#[derive(Debug, Clone)]
pub(crate) enum Update {
    Track(Current),
    #[allow(dead_code)]
    CanGoPrevious(bool),
    Restarted,
    Bookmarked(bool),
    Volume,
    Playback,
    Rate,
    Quit,
}

/// Tells the application whether to forward a handled command to native adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandOutcome {
    /// The command was handled; adapters may now observe its published updates.
    Applied,
    /// The command was not applicable; skip adapter notifications.
    Ignored,
    /// End dispatch; the caller still needs to close the player.
    Quit,
}

/// The playback core.
pub struct Player {
    bookmarks: Bookmarks,
    current: Current,
    downloader: downloader::Handle,
    sink: Arc<rodio::Player>,
    updates: broadcast::Sender<Update>,
    waiter: waiter::Handle,
    history: History,
    /// Identifies the current playback attempt, including restarts.
    /// Replacing audio or entering loading invalidates queued waiter events.
    generation: u64,
}

impl Player {
    pub(crate) async fn new(
        bookmarks: Bookmarks,
        downloader: downloader::Handle,
        sink: Arc<rodio::Player>,
        updates: broadcast::Sender<Update>,
        waiter: waiter::Handle,
    ) -> crate::Result<Self> {
        Ok(Self {
            bookmarks,
            current: Current::default(),
            downloader,
            sink,
            updates,
            waiter,
            history: History::default(),
            generation: 0,
        })
    }

    fn publish(&self, update: Update) -> crate::Result<()> {
        self.updates
            .send(update)
            .map_err(|_| crate::Error::Broadcast)?;
        Ok(())
    }

    fn set_current(&mut self, current: Current) -> crate::Result<()> {
        self.current = current.clone();
        self.publish(Update::Track(current))?;
        if let Current::Track(track) = &self.current {
            let bookmarked = self.bookmarks.bookmarked(track);
            self.publish(Update::Bookmarked(bookmarked))?;
        }
        Ok(())
    }

    /// Close background playback and persist player-owned data.
    #[cfg_attr(test, mutants::skip)]
    pub async fn close(self) -> crate::Result<()> {
        self.sink.stop();
        self.bookmarks.save().await?;
        PersistentVolume::save(self.sink.volume()).await?;
        Ok(())
    }

    /// Includes restart of the first track and restoring retained audio while loading.
    fn can_go_previous(&self) -> bool {
        self.history.has_previous()
            || matches!(self.current, Current::Track(_))
                && History::should_restart(self.sink.get_pos())
            || self.current.loading() && self.history.current().is_some()
    }

    fn update_previous_capability(&self) -> crate::Result<()> {
        self.publish(Update::CanGoPrevious(self.can_go_previous()))
    }

    fn play_current_history(&mut self) -> crate::Result<bool> {
        let Some(track) = self.history.current().cloned() else {
            return Ok(false);
        };
        self.play(track)?;
        Ok(true)
    }

    /// Restarts at or beyond three seconds; otherwise selects a retained previous track.
    /// While loading, restores the retained current track instead.
    fn previous(&mut self) -> crate::Result<()> {
        if self.current.loading() {
            let _ = self.play_current_history()?;
        } else if History::should_restart(self.sink.get_pos()) {
            let _ = self.play_current_history()?;
            self.publish(Update::Restarted)?;
        } else if let Some(track) = self.history.previous().cloned() {
            self.play(track)?;
        }
        self.update_previous_capability()
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

    fn advance(&mut self) -> crate::Result<()> {
        if let Some(track) = self.history.next().cloned() {
            return self.play(track);
        }
        match self.downloader.track() {
            downloader::Output::Loading(progress) => {
                // Invalidate the old playhead before stopping it. Its waiter may
                // still queue completion or capability events while we load.
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

    /// Handles a command, publishing any updates before returning its dispatch outcome.
    /// A handled command may leave values unchanged, such as Play while playing.
    /// Generation checks reject waiter events from a replaced playhead.
    pub(crate) fn handle(&mut self, message: Message) -> crate::Result<CommandOutcome> {
        match message {
            Message::Next if self.current.loading() => return Ok(CommandOutcome::Ignored),
            Message::Next | Message::Init => self.advance()?,
            Message::Loaded if self.current.loading() => self.advance()?,
            Message::Loaded => return Ok(CommandOutcome::Ignored),
            Message::Finished(generation) if generation == self.generation => self.advance()?,
            Message::Finished(_) => return Ok(CommandOutcome::Ignored),
            Message::Previous => self.previous()?,
            Message::PreviousAvailable(generation) if generation == self.generation => {
                self.update_previous_capability()?
            }
            Message::PreviousAvailable(_) => return Ok(CommandOutcome::Ignored),
            Message::Play => {
                self.sink.play();
                self.publish(Update::Playback)?;
            }
            Message::Pause => {
                self.sink.pause();
                self.publish(Update::Playback)?;
            }
            Message::PlayPause => {
                if self.sink.is_paused() {
                    self.sink.play();
                } else {
                    self.sink.pause();
                }
                self.publish(Update::Playback)?;
            }
            Message::ChangeVolume(change) => {
                self.sink
                    .set_volume((self.sink.volume() + change).clamp(0.0, 1.0));
                self.publish(Update::Volume)?;
            }
            Message::SetVolume(volume) => {
                self.sink.set_volume(volume.clamp(0.0, 1.0));
                self.publish(Update::Volume)?;
            }
            Message::SetRate(rate) => {
                self.sink.set_speed(rate);
                self.publish(Update::Rate)?;
            }
            Message::Bookmark => {
                let Current::Track(current) = &self.current else {
                    return Ok(CommandOutcome::Ignored);
                };
                let bookmarked = self.bookmarks.bookmark(current)?;
                self.publish(Update::Bookmarked(bookmarked))?;
            }
            Message::Quit => {
                self.publish(Update::Quit)?;
                return Ok(CommandOutcome::Quit);
            }
        }
        Ok(CommandOutcome::Applied)
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandOutcome, Current, PlaybackView, Player, Update};
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
        let (updates, receiver) = tokio::sync::broadcast::channel(8);
        let (sink, output) = rodio::Player::new();
        (
            Player {
                bookmarks: Bookmarks {
                    entries: Vec::new(),
                },
                current: Default::default(),
                downloader: downloader::Handle::with_tracks(tracks),
                sink: std::sync::Arc::new(sink),
                updates,
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
    ) -> CommandOutcome {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let command = scope.spawn(|| sender.send(player.handle(message)).unwrap());
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
    fn playback_volume_and_rate_publish_ordered_updates() {
        let (mut player, mut first) = player();
        let mut second = first.resubscribe();

        assert_eq!(
            player.handle(Message::Pause).unwrap(),
            CommandOutcome::Applied
        );
        assert!(player.sink.is_paused());
        assert_eq!(
            player.handle(Message::ChangeVolume(-2.0)).unwrap(),
            CommandOutcome::Applied
        );
        assert_eq!(player.sink.volume(), 0.0);
        assert_eq!(
            player.handle(Message::SetVolume(2.0)).unwrap(),
            CommandOutcome::Applied
        );
        assert_eq!(player.sink.volume(), 1.0);
        assert_eq!(
            player.handle(Message::PlayPause).unwrap(),
            CommandOutcome::Applied
        );
        assert!(!player.sink.is_paused());
        assert_eq!(
            player.handle(Message::PlayPause).unwrap(),
            CommandOutcome::Applied
        );
        assert!(player.sink.is_paused());
        assert_eq!(
            player.handle(Message::SetRate(1.5)).unwrap(),
            CommandOutcome::Applied
        );
        assert_eq!(player.sink.speed(), 1.5);
        assert_eq!(PlaybackView::new(player.sink.clone()).rate(), 1.5);
        player.sink.set_volume(0.4);
        player.handle(Message::ChangeVolume(0.1)).unwrap();
        assert_eq!(player.sink.volume(), 0.5);

        for receiver in [&mut first, &mut second] {
            assert!(matches!(receiver.try_recv(), Ok(Update::Playback)));
            assert!(matches!(receiver.try_recv(), Ok(Update::Volume)));
            assert!(matches!(receiver.try_recv(), Ok(Update::Volume)));
            assert!(matches!(receiver.try_recv(), Ok(Update::Playback)));
            assert!(matches!(receiver.try_recv(), Ok(Update::Playback)));
            assert!(matches!(receiver.try_recv(), Ok(Update::Rate)));
            assert!(matches!(receiver.try_recv(), Ok(Update::Volume)));
        }
    }

    #[test]
    fn loading_guards_and_playback_view_reflect_live_state() {
        let tracks = [fixture::queued("one"), fixture::queued("two")];
        let (mut player, mut updates, mut output) = player_with_tracks(tracks);

        assert_eq!(
            player.handle(Message::Next).unwrap(),
            CommandOutcome::Ignored
        );
        assert!(updates.try_recv().is_err());

        assert_eq!(
            player.handle(Message::Loaded).unwrap(),
            CommandOutcome::Applied
        );
        assert_eq!(current_name(&player), "one");
        render(&mut output, 1_024);
        let playback = PlaybackView::new(player.sink.clone());
        assert!(playback.position() > Duration::ZERO);
        assert!(!player.current.loading());
    }

    #[test]
    fn loading_bookmark_is_ignored_and_quit_stops_dispatch() {
        let (mut player, mut receiver) = player();
        assert_eq!(
            player.handle(Message::Bookmark).unwrap(),
            CommandOutcome::Ignored
        );
        assert!(receiver.try_recv().is_err());
        assert_eq!(player.handle(Message::Quit).unwrap(), CommandOutcome::Quit);
        assert!(matches!(receiver.try_recv(), Ok(Update::Quit)));
    }

    #[test]
    fn playback_commands_transition_valid_tracks_and_preserve_pause() {
        let tracks = [fixture::queued("one"), fixture::queued("two")];
        let (mut player, mut updates, mut output) = player_with_tracks(tracks);

        assert_eq!(
            player.handle(Message::Init).unwrap(),
            CommandOutcome::Applied
        );
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

        player.handle(Message::Pause).unwrap();
        assert!(player.sink.is_paused());
        assert!(matches!(updates.try_recv(), Ok(Update::Playback)));
        assert_eq!(
            handle_while_rendering(&mut player, &mut output, Message::Next),
            CommandOutcome::Applied
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

        player.handle(Message::Play).unwrap();
        assert!(!player.sink.is_paused());
        assert!(matches!(updates.try_recv(), Ok(Update::Playback)));
        assert_eq!(
            handle_while_rendering(&mut player, &mut output, Message::Previous),
            CommandOutcome::Applied
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
            CommandOutcome::Applied
        );
        assert_eq!(current_name(&player), "two");
        assert_eq!(player.generation, 4);
    }

    #[test]
    fn matching_generation_advances_and_stale_messages_preserve_transition_state() {
        let tracks = [fixture::queued("one"), fixture::queued("two")];
        let (mut player, mut updates, mut output) = player_with_tracks(tracks);
        player.handle(Message::Init).unwrap();
        render(&mut output, 1_024);
        while updates.try_recv().is_ok() {}
        let first_generation = player.generation;

        assert_eq!(
            handle_while_rendering(
                &mut player,
                &mut output,
                Message::Finished(first_generation)
            ),
            CommandOutcome::Applied
        );
        assert_eq!(current_name(&player), "two");
        let second_generation = player.generation;
        while updates.try_recv().is_ok() {}

        assert_eq!(
            player.handle(Message::Finished(first_generation)).unwrap(),
            CommandOutcome::Ignored
        );
        assert_eq!(
            player
                .handle(Message::PreviousAvailable(first_generation))
                .unwrap(),
            CommandOutcome::Ignored
        );
        assert_eq!(current_name(&player), "two");
        assert_eq!(player.generation, second_generation);
        assert!(updates.try_recv().is_err());

        assert_eq!(
            player
                .handle(Message::PreviousAvailable(second_generation))
                .unwrap(),
            CommandOutcome::Applied
        );
        assert!(matches!(
            updates.try_recv(),
            Ok(Update::CanGoPrevious(true))
        ));
    }

    #[test]
    fn loaded_is_ignored_after_a_track_has_started() {
        let (mut player, mut updates, _) = player_with_tracks([fixture::queued("one")]);
        player.handle(Message::Init).unwrap();
        while updates.try_recv().is_ok() {}

        assert_eq!(
            player.handle(Message::Loaded).unwrap(),
            CommandOutcome::Ignored
        );
        assert_eq!(current_name(&player), "one");
        assert!(updates.try_recv().is_err());
    }

    #[test]
    fn previous_restarts_a_rendered_track_after_three_seconds() {
        let (mut player, mut updates, mut output) = player_with_tracks([fixture::queued("one")]);
        player.handle(Message::Init).unwrap();
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
    fn publishing_without_a_subscriber_is_an_error() {
        let (mut player, receiver) = player();
        drop(receiver);
        assert!(player.handle(Message::Pause).is_err());
    }

    #[test]
    fn stale_generation_messages_are_ignored() {
        let (mut player, mut receiver) = player();
        assert_eq!(
            player.handle(Message::Finished(1)).unwrap(),
            CommandOutcome::Ignored
        );
        assert_eq!(
            player.handle(Message::PreviousAvailable(1)).unwrap(),
            CommandOutcome::Ignored
        );
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn loading_invalidates_completion_and_previous_restores_retained_audio() {
        let (mut player, mut updates, mut output) = player_with_tracks([fixture::queued("one")]);
        player.handle(Message::Init).unwrap();
        render(&mut output, 1_024);
        let generation = player.generation;
        handle_while_rendering(&mut player, &mut output, Message::Next);
        assert!(player.current.loading());
        assert_ne!(player.generation, generation);
        while updates.try_recv().is_ok() {}
        assert_eq!(
            player.handle(Message::Finished(generation)).unwrap(),
            CommandOutcome::Ignored
        );
        assert_eq!(
            player
                .handle(Message::PreviousAvailable(generation))
                .unwrap(),
            CommandOutcome::Ignored
        );
        assert!(updates.try_recv().is_err());
        handle_while_rendering(&mut player, &mut output, Message::Previous);
        assert_eq!(current_name(&player), "one");
        assert_eq!(
            player.handle(Message::Loaded).unwrap(),
            CommandOutcome::Ignored
        );
    }

    #[test]
    fn track_changes_preserve_pause_and_publish_metadata() {
        let (mut player, mut updates, mut output) =
            player_with_tracks([fixture::queued("one"), fixture::queued("two")]);
        player.handle(Message::Init).unwrap();
        assert_eq!(current_name(&player), "one");
        assert!(matches!(
            updates.try_recv(),
            Ok(Update::Track(Current::Track(_)))
        ));
        assert!(matches!(updates.try_recv(), Ok(Update::Bookmarked(false))));
        assert!(matches!(
            updates.try_recv(),
            Ok(Update::CanGoPrevious(false))
        ));
        render(&mut output, 1_024);
        player.handle(Message::Pause).unwrap();
        assert!(matches!(updates.try_recv(), Ok(Update::Playback)));
        assert_eq!(
            handle_while_rendering(&mut player, &mut output, Message::Next),
            CommandOutcome::Applied
        );
        assert_eq!(current_name(&player), "two");
        assert!(player.sink.is_paused());
        assert!(matches!(
            updates.try_recv(),
            Ok(Update::Track(Current::Track(_)))
        ));
        assert!(matches!(updates.try_recv(), Ok(Update::Bookmarked(false))));
        assert!(matches!(
            updates.try_recv(),
            Ok(Update::CanGoPrevious(true))
        ));
    }
}
