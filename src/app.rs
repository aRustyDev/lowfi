//! Application composition and command routing.

use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};

use crate::{
    bookmark::Bookmarks,
    config,
    player::{CommandOutcome, PlaybackView, Player, State, Update},
    tracks, ui,
    volume::PersistentVolume,
    Message, Tasks,
};

pub(crate) struct Application {
    player: Player,
    tasks: Tasks,
    commands: mpsc::Receiver<Message>,
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
        let state = State::initial(PlaybackView::new(Arc::clone(&sink)), list.name.clone());

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

        if settings.playback.paused {
            sender.send(Message::Pause).await?;
        }
        sender.send(Message::Init).await?;

        Ok(Self {
            player,
            tasks,
            commands,
            updates: receiver,
            #[cfg(feature = "mpris")]
            mpris,
            #[cfg(all(feature = "media-controls", windows))]
            media_controls,
        })
    }

    pub(crate) async fn run(&mut self) -> crate::Result<()> {
        loop {
            tokio::select! {
                result = &mut self.tasks => return result,
                message = self.commands.recv() => {
                    let Some(message) = message else { return Ok(()) };
                    match self.player.handle(message)? {
                        CommandOutcome::Applied => {
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
