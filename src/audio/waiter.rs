//! Watches the current playback generation without owning an audio device.

use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{mpsc, watch},
    time,
};

async fn waiter(
    sink: Arc<rodio::Player>,
    tx: mpsc::Sender<crate::Message>,
    mut generations: watch::Receiver<u64>,
) -> crate::Result<()> {
    while generations.changed().await.is_ok() {
        let mut generation = *generations.borrow_and_update();
        let mut previous_announced = false;
        loop {
            tokio::select! {
                changed = generations.changed() => {
                    if changed.is_err() { return Ok(()); }
                    generation = *generations.borrow_and_update();
                    previous_announced = false;
                }
                _ = time::sleep(Duration::from_millis(16)) => {
                    if !previous_announced && sink.get_pos() >= crate::history::History::RESTART_AFTER {
                        tx.send(crate::Message::PreviousAvailable(generation)).await?;
                        previous_announced = true;
                    }
                    if sink.empty() {
                        tx.send(crate::Message::Finished(generation)).await?;
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Publishes the generation whose playback should be observed.
pub struct Handle {
    generations: watch::Sender<u64>,
}

impl Handle {
    #[cfg(test)]
    pub(crate) fn idle() -> Self {
        let (generations, _) = watch::channel(0);
        Self { generations }
    }

    pub fn notify(&self, generation: u64) {
        self.generations.send_replace(generation);
    }
}

impl crate::Tasks {
    pub fn waiter(&mut self, sink: Arc<rodio::Player>) -> Handle {
        let (generations, receiver) = watch::channel(0);
        self.spawn(waiter(sink, self.tx(), receiver));
        Handle { generations }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{tests::fixture, Message};

    #[tokio::test]
    async fn observes_restart_threshold_and_completion_from_real_playback() {
        let (sink, mut output) = rodio::Player::new();
        let sink = Arc::new(sink);
        sink.append(fixture::queued("waiter").decode().unwrap().data);
        let (sender, mut messages) = mpsc::channel(8);
        let (generations, receiver) = watch::channel(0);
        let task = tokio::spawn(waiter(sink.clone(), sender, receiver));
        let handle = Handle { generations };
        handle.notify(7);
        for _ in 0..160_000 {
            output.next().unwrap();
        }
        assert!(sink.get_pos() >= crate::history::History::RESTART_AFTER);
        assert_eq!(
            time::timeout(Duration::from_secs(1), messages.recv())
                .await
                .unwrap(),
            Some(Message::PreviousAvailable(7))
        );
        for _ in 0..100_000 {
            output.next().unwrap();
        }
        assert!(sink.empty());
        assert_eq!(
            time::timeout(Duration::from_secs(1), messages.recv())
                .await
                .unwrap(),
            Some(Message::Finished(7))
        );
        assert!(messages.try_recv().is_err());
        drop(handle);
        time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn replaces_pending_generations_and_closes_cleanly() {
        let (sink, _) = rodio::Player::new();
        let (sender, mut messages) = mpsc::channel(8);
        let (generations, receiver) = watch::channel(0);
        let task = tokio::spawn(waiter(Arc::new(sink), sender, receiver));
        let handle = Handle { generations };
        handle.notify(1);
        tokio::task::yield_now().await;
        handle.notify(2);
        assert_eq!(
            time::timeout(Duration::from_secs(1), messages.recv())
                .await
                .unwrap(),
            Some(Message::Finished(2))
        );
        assert!(messages.try_recv().is_err());
        drop(handle);
        time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}
