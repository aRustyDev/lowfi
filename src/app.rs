//! Application composition and command routing.

use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, watch};

use crate::{
    bookmark::Bookmarks,
    config,
    player::{self, CommandOutcome, PlaybackView, Player, State, Update},
    tracks, ui,
    volume::PersistentVolume,
    Message, Tasks,
};

/// Owns one playback session, its tasks, and the command path shared by frontends.
pub(crate) struct Application {
    player: Player,
    tasks: Tasks,
    commands: mpsc::Receiver<Message>,
    #[cfg_attr(not(all(feature = "menubar", target_os = "macos")), allow(dead_code))]
    sender: mpsc::Sender<Message>,
    #[cfg_attr(not(all(feature = "menubar", target_os = "macos")), allow(dead_code))]
    snapshot: watch::Sender<player::PlaybackSnapshot>,
    track_list: String,
    /// Keeps a subscriber alive for the application lifetime. Publishing fails
    /// if every receiver is dropped, even when no frontend consumes updates.
    #[allow(dead_code)]
    updates: broadcast::Receiver<Update>,
    #[cfg(feature = "mpris")]
    mpris: ui::mpris::Server,
    #[cfg(all(feature = "media-controls", windows))]
    media_controls: Option<ui::media_controls::Server>,
}

impl Application {
    pub(crate) async fn init(
        settings: config::Settings,
        bindings: config::BindingMap,
        mixer: &rodio::mixer::Mixer,
        terminal: bool,
    ) -> crate::Result<Self> {
        let (sender, commands) = mpsc::channel(8);
        let mut tasks = Tasks::new(sender.clone());
        let (updates, receiver) = broadcast::channel(8);

        let list = tracks::List::load(&settings.playback.track_list).await?;
        let sink = Arc::new(rodio::Player::connect_new(mixer));
        sink.set_volume(PersistentVolume::load().await?.float());
        let track_list = list.name.clone();
        let state = State::initial(PlaybackView::new(Arc::clone(&sink)), track_list.clone());

        let logger = ui::init::terminal(
            &mut tasks,
            state.clone(),
            &settings,
            bindings,
            receiver.resubscribe(),
            terminal,
        )?;

        #[cfg(feature = "mpris")]
        let mpris =
            ui::mpris::Server::new(state.clone(), sender.clone(), receiver.resubscribe()).await?;
        #[cfg(all(feature = "media-controls", windows))]
        let media_controls = ui::media_controls::Server::try_new(
            state.clone(),
            sender.clone(),
            receiver.resubscribe(),
        );

        let downloader = tasks.downloader(&settings, logger, list)?;
        let waiter = tasks.waiter(Arc::clone(&sink));
        let player =
            Player::new(Bookmarks::load().await?, downloader, sink, updates, waiter).await?;
        let (snapshot, _) = watch::channel(player.snapshot(track_list.clone()));

        if settings.playback.paused {
            sender.send(Message::Pause).await?;
        }
        sender.send(Message::Init).await?;

        Ok(Self {
            player,
            tasks,
            commands,
            sender,
            snapshot,
            track_list,
            updates: receiver,
            #[cfg(feature = "mpris")]
            mpris,
            #[cfg(all(feature = "media-controls", windows))]
            media_controls,
        })
    }

    #[cfg_attr(not(all(feature = "menubar", target_os = "macos")), allow(dead_code))]
    pub(crate) fn sender(&self) -> mpsc::Sender<Message> {
        self.sender.clone()
    }

    /// Subscribes to the latest playback state, refreshed by applied commands.
    ///
    /// The watch channel may coalesce updates; consume snapshots as state, not an
    /// event log. Notifications do not provide a periodic playback-progress timer.
    #[cfg_attr(not(all(feature = "menubar", target_os = "macos")), allow(dead_code))]
    pub(crate) fn subscribe(&self) -> watch::Receiver<player::PlaybackSnapshot> {
        self.snapshot.subscribe()
    }

    fn publish_snapshot(&self) {
        self.snapshot
            .send_replace(self.player.snapshot(self.track_list.clone()));
    }

    /// Dispatches commands until quit, channel closure, task completion, or an error.
    /// Native adapters observe applied commands after the core publishes updates.
    /// The caller remains responsible for terminal cleanup and [`Self::close`].
    pub(crate) async fn run(&mut self) -> crate::Result<()> {
        loop {
            tokio::select! {
                result = &mut self.tasks => return result,
                message = self.commands.recv() => {
                    let Some(message) = message else { return Ok(()) };
                    match self.player.handle(message)? {
                        CommandOutcome::Applied => {
                            self.publish_snapshot();
                            #[cfg(feature = "mpris")]
                            self.mpris.handle(&message).await?;
                            #[cfg(all(feature = "media-controls", windows))]
                            if let Some(media_controls) = &mut self.media_controls {
                                media_controls.handle(&message)?;
                            }
                        }
                        CommandOutcome::Ignored => {}
                        CommandOutcome::Quit => return Ok(()),
                    }
                }
            }
        }
    }

    pub(crate) async fn close(self) -> crate::Result<()> {
        self.player.close().await
    }
}
