//! Bounded session-local compressed track history.

use crate::tracks;
use std::{collections::VecDeque, time::Duration};

/// A bounded timeline of compressed tracks retained for this session.
///
/// The eleven-entry limit includes the current track and any forward entries.
/// Moving backward keeps those entries: after A → B → C, Previous selects B
/// and Next returns C without downloading it again.
#[derive(Default)]
pub(crate) struct History {
    tracks: VecDeque<tracks::Queued>,
    current: Option<usize>,
}

impl History {
    /// Maximum retained entries, including the current one.
    pub(crate) const CAPACITY: usize = 11;
    /// At this inclusive threshold, Previous restarts rather than moving backward.
    pub(crate) const RESTART_AFTER: Duration = Duration::from_secs(3);
    pub(crate) fn current(&self) -> Option<&tracks::Queued> {
        self.current.and_then(|index| self.tracks.get(index))
    }
    /// Moves the cursor backward without removing retained forward entries.
    pub(crate) fn previous(&mut self) -> Option<&tracks::Queued> {
        let index = self.current?.checked_sub(1)?;
        self.current = Some(index);
        self.current()
    }
    pub(crate) fn next(&mut self) -> Option<&tracks::Queued> {
        let index = self.current?.checked_add(1)?;
        if index >= self.tracks.len() {
            return None;
        }
        self.current = Some(index);
        self.current()
    }
    /// Appends and selects a downloaded track, evicting the oldest if full.
    ///
    /// The player exhausts retained forward entries with `next` before calling
    /// this. Appending does not truncate entries beyond the current cursor.
    pub(crate) fn push(&mut self, track: tracks::Queued) {
        self.tracks.push_back(track);
        self.current = Some(self.tracks.len() - 1);
        if self.tracks.len() > Self::CAPACITY {
            self.tracks.pop_front();
            self.current = self.current.map(|index| index - 1);
        }
    }
    pub(crate) fn has_previous(&self) -> bool {
        self.current.is_some_and(|index| index > 0)
    }
    pub(crate) fn should_restart(position: Duration) -> bool {
        position >= Self::RESTART_AFTER
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracks::Queued;
    use bytes::Bytes;

    fn track(name: &str) -> Queued {
        Queued::new(name.into(), Bytes::from_static(b"audio"), Some(name.into())).unwrap()
    }

    #[test]
    fn history_is_bounded_and_evicts_oldest_track() {
        let mut history = History::default();
        for index in 0..=History::CAPACITY {
            history.push(track(&index.to_string()));
        }
        assert_eq!(history.tracks.len(), History::CAPACITY);
        assert_eq!(
            history.current().unwrap().display,
            History::CAPACITY.to_string()
        );
        assert_eq!(
            history.previous().unwrap().display,
            (History::CAPACITY - 1).to_string()
        );
        while history.previous().is_some() {}
        assert_eq!(history.current().unwrap().display, "1");
    }

    #[test]
    fn history_keeps_forward_entries_after_going_back() {
        let mut history = History::default();
        history.push(track("one"));
        history.push(track("two"));
        history.push(track("three"));
        assert_eq!(history.previous().unwrap().display, "two");
        assert_eq!(history.next().unwrap().display, "three");
        assert!(history.next().is_none());
    }

    #[test]
    fn history_has_no_previous_at_startup() {
        let mut history = History::default();
        assert!(history.current().is_none());
        assert!(history.previous().is_none());
        assert!(!history.has_previous());
    }

    #[test]
    fn previous_restart_threshold_is_three_seconds() {
        assert!(!History::should_restart(Duration::from_millis(2999)));
        assert!(History::should_restart(Duration::from_secs(3)));
    }

    #[test]
    fn history_returns_to_skipped_track_before_downloading() {
        let mut history = History::default();
        history.push(track("one"));
        history.push(track("two"));
        history.push(track("three"));
        assert_eq!(history.previous().unwrap().display, "two");
        assert_eq!(history.previous().unwrap().display, "one");
        assert_eq!(history.next().unwrap().display, "two");
        assert_eq!(history.next().unwrap().display, "three");
        assert!(history.next().is_none());
        assert_eq!(history.current().unwrap().display, "three");
        assert_eq!(history.tracks.len(), 3);
    }
}
