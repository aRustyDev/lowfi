//! Contains the code for the MPRIS server & other helper functions.

use std::{
    hash::{DefaultHasher, Hash, Hasher},
    process,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use arc_swap::ArcSwap;
use mpris_server::{
    zbus::{self, fdo, Result},
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus, PlayerInterface, Property, RootInterface,
    Signal, Time, TrackId, Volume,
};
use tokio::sync::{broadcast, mpsc};

use crate::player::{Current, PlaybackView, State, Update};
use crate::{ui, Message};

const ERROR: fdo::Error = fdo::Error::Failed(String::new());

struct Sender {
    inner: mpsc::Sender<Message>,
}

impl Sender {
    pub fn new(inner: mpsc::Sender<Message>) -> Self {
        Self { inner }
    }

    pub async fn send(&self, message: Message) -> fdo::Result<()> {
        self.inner
            .send(message)
            .await
            .map_err(|x| fdo::Error::Failed(x.to_string()))
    }

    pub async fn zbus(&self, message: Message) -> zbus::Result<()> {
        self.inner
            .send(message)
            .await
            .map_err(|x| zbus::Error::Failure(x.to_string()))
    }
}

impl Into<fdo::Error> for crate::Error {
    fn into(self) -> fdo::Error {
        fdo::Error::Failed(self.to_string())
    }
}

/// The actual MPRIS player.
pub struct Player {
    playback: PlaybackView,
    current: ArcSwap<Current>,
    list: String,
    sender: Sender,
    can_go_previous: Arc<AtomicBool>,
}

impl RootInterface for Player {
    async fn raise(&self) -> fdo::Result<()> {
        Err(ERROR)
    }

    async fn quit(&self) -> fdo::Result<()> {
        self.sender.send(Message::Quit).await
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _: bool) -> Result<()> {
        Ok(())
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn identity(&self) -> fdo::Result<String> {
        Ok("lowfi".to_owned())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok("dev.talwat.lowfi".to_owned())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["https".to_owned()])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["audio/mpeg".to_owned()])
    }
}

impl PlayerInterface for Player {
    async fn next(&self) -> fdo::Result<()> {
        self.sender.send(Message::Next).await
    }

    async fn previous(&self) -> fdo::Result<()> {
        if self.can_go_previous.load(Ordering::Relaxed) {
            self.sender.send(Message::Previous).await?;
        }
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.sender.send(Message::Pause).await
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        self.sender.send(Message::PlayPause).await
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.sender.send(Message::Quit).await
    }

    async fn play(&self) -> fdo::Result<()> {
        self.sender.send(Message::Play).await
    }

    async fn seek(&self, _offset: Time) -> fdo::Result<()> {
        Err(ERROR)
    }

    async fn set_position(&self, _track_id: TrackId, _position: Time) -> fdo::Result<()> {
        Err(ERROR)
    }

    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        Err(ERROR)
    }

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        Ok(if self.current.load().loading() {
            PlaybackStatus::Stopped
        } else if self.playback.is_paused() {
            PlaybackStatus::Paused
        } else {
            PlaybackStatus::Playing
        })
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Err(ERROR)
    }

    async fn set_loop_status(&self, _loop_status: LoopStatus) -> Result<()> {
        Ok(())
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(self.playback.rate().into())
    }

    async fn set_rate(&self, rate: PlaybackRate) -> Result<()> {
        self.sender.zbus(Message::SetRate(rate as f32)).await
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn set_shuffle(&self, _shuffle: bool) -> Result<()> {
        Ok(())
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        Ok(match self.current.load().as_ref() {
            Current::Loading(_) => Metadata::new(),
            Current::Track(track) => {
                let mut hasher = DefaultHasher::new();
                track.path.hash(&mut hasher);

                let id = mpris_server::zbus::zvariant::ObjectPath::try_from(format!(
                    "/com/talwat/lowfi/{}/{}",
                    self.list,
                    hasher.finish()
                ))
                .unwrap();

                let mut metadata = Metadata::builder()
                    .trackid(id)
                    .title(track.display.clone())
                    .album(self.list.clone())
                    .build();

                metadata.set_length(
                    track
                        .duration
                        .map(|x| Time::from_micros(x.as_micros() as i64)),
                );

                metadata
            }
        })
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        Ok(self.playback.volume().into())
    }

    async fn set_volume(&self, volume: Volume) -> Result<()> {
        self.sender.zbus(Message::SetVolume(volume as f32)).await
    }

    async fn position(&self) -> fdo::Result<Time> {
        Ok(Time::from_micros(
            self.playback.position().as_micros() as i64
        ))
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(0.2f64)
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(3.0f64)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(self.can_go_previous.load(Ordering::Relaxed))
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}

/// A struct which contains the MPRIS [Server], and has some helper functions
/// to make it easier to work with.
pub struct Server {
    /// The inner MPRIS server.
    inner: mpris_server::Server<Player>,

    /// Broadcast receiver.
    receiver: broadcast::Receiver<Update>,
}

impl Server {
    /// Handles a player message to update the state of the MPRIS player.
    pub async fn handle(&mut self, message: &crate::Message) -> ui::Result<()> {
        let mut restarted = false;
        let mut capability_changed = false;
        while let Ok(update) = self.receiver.try_recv() {
            match update {
                Update::Track(current) => {
                    self.player().current.swap(Arc::new(current));
                }
                Update::CanGoPrevious(can_go_previous) => {
                    self.player()
                        .can_go_previous
                        .store(can_go_previous, Ordering::Relaxed);
                    capability_changed = true;
                }
                Update::Restarted => restarted = true,
                _ => (),
            }
        }

        let result = match message {
            Message::SetRate(_) => self.update_rate().await,
            Message::ChangeVolume(_) | Message::SetVolume(_) => self.update_volume().await,
            Message::Play | Message::Pause | Message::PlayPause => self.update_playback().await,
            Message::Init
            | Message::Loaded
            | Message::Next
            | Message::Previous
            | Message::Finished(_) => self.update_metadata().await,
            _ => Ok(()),
        };
        result?;
        if capability_changed {
            self.update_previous_capability().await?;
        }
        if restarted {
            self.inner
                .emit(Signal::Seeked {
                    position: Time::from_micros(0),
                })
                .await?;
        }
        Ok(())
    }

    /// Shorthand to emit a `PropertiesChanged` signal, like when pausing/unpausing.
    async fn changed(
        &mut self,
        properties: impl IntoIterator<Item = mpris_server::Property> + Send + Sync,
    ) -> ui::Result<()> {
        self.inner.properties_changed(properties).await?;
        Ok(())
    }

    /// Updates the volume with the latest information.
    async fn update_volume(&mut self) -> ui::Result<()> {
        self.changed(vec![Property::Volume(
            self.player().playback.volume().into(),
        )])
        .await?;

        Ok(())
    }

    /// Updates the playback with the latest information.
    async fn update_playback(&mut self) -> ui::Result<()> {
        let status = self.player().playback_status().await?;
        self.changed(vec![Property::PlaybackStatus(status)]).await?;

        Ok(())
    }

    async fn update_rate(&mut self) -> ui::Result<()> {
        self.changed(vec![Property::Rate(self.player().playback.rate().into())])
            .await
    }

    async fn update_previous_capability(&mut self) -> ui::Result<()> {
        self.changed(vec![Property::CanGoPrevious(
            self.player().can_go_previous.load(Ordering::Relaxed),
        )])
        .await
    }

    /// Updates the current track data with the current information.
    async fn update_metadata(&mut self) -> ui::Result<()> {
        let metadata = self.player().metadata().await?;
        let status = self.player().playback_status().await?;
        self.changed(vec![
            Property::Metadata(metadata),
            Property::PlaybackStatus(status),
            Property::CanGoPrevious(self.player().can_go_previous.load(Ordering::Relaxed)),
        ])
        .await?;

        Ok(())
    }

    /// Shorthand to get the inner mpris player object.
    pub(crate) fn player(&self) -> &Player {
        self.inner.imp()
    }

    /// Creates a new MPRIS server.
    pub(crate) async fn new(
        state: State,
        sender: mpsc::Sender<Message>,
        receiver: broadcast::Receiver<Update>,
    ) -> ui::Result<Server> {
        let suffix = if crate::env("LOWFI_FIXED_MPRIS_NAME") {
            String::from("lowfi")
        } else {
            format!("lowfi.{}.instance{}", state.track_list, process::id())
        };

        // Filtered track list, since MPRIS objects don't like non-alphanumeric characters.
        let tracklist: String = state
            .track_list
            .chars()
            .filter(|c| c.is_ascii_lowercase())
            .collect();

        let server = mpris_server::Server::new(
            &suffix,
            Player {
                sender: Sender::new(sender),
                playback: state.playback,
                current: ArcSwap::new(Arc::new(state.current)),
                list: tracklist,
                can_go_previous: Arc::new(AtomicBool::new(false)),
            },
        )
        .await?;

        Ok(Self {
            inner: server,
            receiver,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Call the interface methods directly to test Lowfi's command/state contract.
    //! No D-Bus service is registered; bus traffic and property signals need live
    //! integration validation beyond these in-process checks.

    use super::*;
    use crate::{tests::fixture, tracks::Info};
    use rodio::queue::SourcesQueueOutput;
    use std::{sync::Arc, time::Duration};

    fn player(
        can_go_previous: bool,
    ) -> (
        Player,
        mpsc::Receiver<Message>,
        Arc<rodio::Player>,
        SourcesQueueOutput,
    ) {
        let (sink, output) = rodio::Player::new();
        let sink = Arc::new(sink);
        let (sender, receiver) = mpsc::channel(1);
        (
            Player {
                playback: PlaybackView::new(sink.clone()),
                current: ArcSwap::new(Arc::new(Current::default())),
                list: String::from("test"),
                sender: Sender::new(sender),
                can_go_previous: Arc::new(AtomicBool::new(can_go_previous)),
            },
            receiver,
            sink,
            output,
        )
    }

    #[tokio::test]
    async fn previous_capability_and_dispatch_follow_history_state() {
        let (player, mut receiver, _, _) = player(false);
        assert!(!player.can_go_previous().await.unwrap());
        player.previous().await.unwrap();
        assert!(receiver.try_recv().is_err());

        player.can_go_previous.store(true, Ordering::Relaxed);
        assert!(player.can_go_previous().await.unwrap());
        player.previous().await.unwrap();
        assert_eq!(receiver.try_recv(), Ok(Message::Previous));
    }

    #[tokio::test]
    async fn rate_changes_are_forwarded_to_the_command_channel() {
        let (player, mut receiver, _, _) = player(false);
        player.set_rate(1.25).await.unwrap();
        assert_eq!(receiver.try_recv(), Ok(Message::SetRate(1.25)));
    }

    #[tokio::test]
    async fn playback_commands_are_forwarded_to_the_command_channel() {
        let (player, mut receiver, _, _) = player(false);
        player.play().await.unwrap();
        assert_eq!(receiver.try_recv(), Ok(Message::Play));
        player.pause().await.unwrap();
        assert_eq!(receiver.try_recv(), Ok(Message::Pause));
        player.play_pause().await.unwrap();
        assert_eq!(receiver.try_recv(), Ok(Message::PlayPause));
        player.next().await.unwrap();
        assert_eq!(receiver.try_recv(), Ok(Message::Next));
        player.stop().await.unwrap();
        assert_eq!(receiver.try_recv(), Ok(Message::Quit));
        player.quit().await.unwrap();
        assert_eq!(receiver.try_recv(), Ok(Message::Quit));
        player.set_volume(0.25).await.unwrap();
        assert_eq!(receiver.try_recv(), Ok(Message::SetVolume(0.25)));
    }

    #[tokio::test]
    async fn closed_command_channel_returns_an_error() {
        let (player, receiver, _, _) = player(false);
        drop(receiver);
        assert!(player.play().await.is_err());
        assert!(player.set_rate(1.25).await.is_err());
    }

    #[tokio::test]
    async fn player_properties_and_metadata_follow_playback_state() {
        let (player, _, sink, mut output) = player(true);
        assert!(player.raise().await.is_err());
        assert!(player.can_quit().await.unwrap());
        assert!(!player.fullscreen().await.unwrap());
        player.set_fullscreen(true).await.unwrap();
        assert!(!player.can_set_fullscreen().await.unwrap());
        assert!(!player.can_raise().await.unwrap());
        assert!(!player.has_track_list().await.unwrap());
        assert_eq!(player.identity().await.unwrap(), "lowfi");
        assert_eq!(player.desktop_entry().await.unwrap(), "dev.talwat.lowfi");
        assert_eq!(
            player.supported_uri_schemes().await.unwrap(),
            vec!["https".to_owned()]
        );
        assert_eq!(
            player.supported_mime_types().await.unwrap(),
            vec!["audio/mpeg".to_owned()]
        );
        assert_eq!(
            player.playback_status().await.unwrap(),
            PlaybackStatus::Stopped
        );
        assert!(player.seek(Time::ZERO).await.is_err());
        assert!(player
            .set_position(TrackId::NO_TRACK, Time::ZERO)
            .await
            .is_err());
        assert!(player
            .open_uri("https://example.invalid".into())
            .await
            .is_err());
        assert!(player.loop_status().await.is_err());
        player.set_loop_status(LoopStatus::None).await.unwrap();
        assert!(player.shuffle().await.unwrap());
        player.set_shuffle(false).await.unwrap();
        assert_eq!(player.volume().await.unwrap(), 1.0);
        assert_eq!(player.rate().await.unwrap(), 1.0);
        assert_eq!(player.position().await.unwrap(), Time::ZERO);
        assert_eq!(player.minimum_rate().await.unwrap(), 0.2);
        assert_eq!(player.maximum_rate().await.unwrap(), 3.0);
        assert!(player.can_go_next().await.unwrap());
        assert!(player.can_go_previous().await.unwrap());
        assert!(player.can_play().await.unwrap());
        assert!(player.can_pause().await.unwrap());
        assert!(!player.can_seek().await.unwrap());
        assert!(player.can_control().await.unwrap());

        sink.append(fixture::queued("position").decode().unwrap().data);
        for _ in 0..1_024 {
            assert!(output.next().is_some_and(|sample| sample.is_finite()));
        }
        assert!(player.position().await.unwrap() > Time::ZERO);

        player.current.store(Arc::new(Current::Track(Info {
            path: "https://example.invalid/tone.mp3".into(),
            display: "Test Tone".into(),
            width: 9,
            duration: Some(Duration::from_secs(4)),
        })));
        let metadata = player.metadata().await.unwrap();
        assert_eq!(metadata.title(), Some("Test Tone"));
        assert_eq!(metadata.album(), Some("test"));
        assert_eq!(metadata.length(), Some(Time::from_secs(4)));
        assert!(metadata.trackid().is_some());
        assert_eq!(
            player.playback_status().await.unwrap(),
            PlaybackStatus::Playing
        );
    }
}
