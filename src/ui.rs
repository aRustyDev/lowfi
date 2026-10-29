//! Terminal input and rendering.

use tokio::{sync::broadcast, time::Instant};

use crate::player::{self, Update};

pub mod environment;
pub use environment::Environment;
pub mod init;
pub mod input;
pub mod interface;
pub use interface::Interface;

#[cfg(feature = "mpris")]
pub mod mpris;

#[cfg(all(feature = "media-controls", windows))]
pub mod media_controls;

pub(crate) type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unable to convert number: {0}")]
    Conversion(#[from] std::num::TryFromIntError),
    #[error("unable to write output: {0}")]
    Write(#[from] std::io::Error),
    #[error("sending signal message to backend from ui failed: {0}")]
    SignalSend(#[from] tokio::sync::mpsc::error::SendError<crate::Message>),
    #[error("error sending a log: {0}")]
    LogSend(#[from] tokio::sync::mpsc::error::SendError<String>),
    #[error("you can't disable the UI without MPRIS!")]
    RejectedDisable,
    #[cfg(feature = "mpris")]
    #[error("mpris bus error: {0}")]
    ZBus(#[from] mpris_server::zbus::Error),
    #[cfg(feature = "mpris")]
    #[error("mpris fdo (zbus interface) error: {0}")]
    Fdo(#[from] mpris_server::zbus::fdo::Error),
    #[cfg(all(feature = "media-controls", windows))]
    #[error("media controls error: {0}")]
    MediaControls(String),
}

/// Terminal-only state layered over the neutral playback state.
pub(crate) struct State {
    pub(crate) player: player::State,
    pub(crate) volume_timer: Option<Instant>,
}

impl State {
    pub(crate) fn new(player: player::State) -> Self {
        Self {
            player,
            volume_timer: None,
        }
    }

    pub(crate) fn tick(&mut self) {
        if self
            .volume_timer
            .is_some_and(|timer| timer.elapsed() > std::time::Duration::from_secs(1))
        {
            self.volume_timer = None;
        }
    }

    fn apply(&mut self, update: Update) -> bool {
        match update {
            Update::Track(track) => self.player.current = track,
            Update::Bookmarked(bookmarked) => self.player.bookmarked = bookmarked,
            Update::Volume => self.volume_timer = Some(Instant::now()),
            Update::Quit => return false,
            Update::CanGoPrevious(_) | Update::Restarted | Update::Playback | Update::Rate => {}
        }
        true
    }
}

/// Renders terminal state and consumes core updates at the existing frame cadence.
pub(crate) async fn run(
    mut updater: broadcast::Receiver<Update>,
    mut interface: Interface,
    mut state: State,
) -> Result<()> {
    loop {
        if let Ok(update) = updater.try_recv() {
            if !state.apply(update) {
                break;
            }
        }
        interface.draw(&state).await?;
        state.tick();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::{PlaybackView, State as PlayerState};
    use std::sync::Arc;

    #[test]
    fn only_volume_updates_start_the_volume_timer() {
        let sink = Arc::new(rodio::Player::new().0);
        let mut state = State::new(PlayerState::initial(PlaybackView::new(sink), "test".into()));

        state.apply(Update::Playback);
        state.apply(Update::Rate);
        assert!(state.volume_timer.is_none());
        state.apply(Update::Volume);
        assert!(state.volume_timer.is_some());
    }
}
