//! Has all of the structs for managing the state
//! of tracks, as well as downloading them & finding new ones.
//!
//! There are several structs which represent the different stages
//! that go on in downloading and playing tracks. When first queued,
//! the downloader will return a [`Queued`] track.
//!
//! Then, when it's time to play the track, it is decoded into
//! a [`Decoded`] track, which includes all the information
//! in the form of [`Info`].

use std::{fmt::Debug, io::Cursor, time::Duration};

use bytes::Bytes;
use rodio::{Decoder, Source as _};
use unicode_segmentation::UnicodeSegmentation as _;

pub mod list;
pub use list::List;
pub mod error;
pub mod format;
pub use error::{Error, Result};

use crate::tracks::error::WithTrackContext as _;

/// Lazy audio decoder backed by in-memory compressed [`Bytes`].
pub type DecodedData = Decoder<Cursor<Bytes>>;

/// Tracks which are still waiting in the queue, and can't be played yet.
///
/// This means that only the data & track name are included.
#[derive(PartialEq, Eq)]
pub struct Queued {
    /// Display name of the track.
    pub display: String,

    /// Full downloadable path/url of the track.
    pub path: String,

    /// The raw data of the track, which is not decoded and
    /// therefore much more memory efficient.
    pub data: Bytes,
}

impl Debug for Queued {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Queued")
            .field("display", &self.display)
            .field("path", &self.path)
            .field("data", &self.data.len())
            .finish()
    }
}

impl Queued {
    /// Decodes the stored audio and derives playback metadata in a [`Decoded`].
    /// Returns an error if the audio cannot be decoded.
    pub fn decode(self) -> Result<Decoded> {
        Decoded::new(self)
    }

    /// Creates a new queued track.
    pub fn new(path: String, data: Bytes, display: Option<String>) -> Result<Self> {
        let display = match display {
            None => self::format::name(&path)?,
            Some(custom) => custom,
        };

        Ok(Self {
            display,
            path,
            data,
        })
    }
}

/// Track metadata derived when compressed audio is prepared for playback.
/// Duration is discovered through the decoder, so it is not part of [`Queued`].
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Info {
    /// The full downloadable path/url of the track.
    pub path: String,

    /// This is a formatted name, so it doesn't include the full path.
    pub display: String,

    /// Grapheme count used to size the track name in the terminal UI.
    pub width: usize,

    /// The duration of the track, this is an [Option] because there are
    /// cases where the duration of a track is unknown.
    pub duration: Option<Duration>,
}

impl Info {
    /// Converts the info back into a full track list entry.
    pub fn to_entry(&self) -> String {
        let mut entry = self.path.clone();
        entry.push('!');
        entry.push_str(&self.display);

        entry
    }

    /// Creates a new [`Info`] from decoded data & the queued track.
    pub fn new(decoded: &DecodedData, path: String, display: String) -> Result<Self> {
        Ok(Self {
            duration: decoded.total_duration(),
            width: display.graphemes(true).count(),
            path,
            display,
        })
    }
}

/// Audio decoder and metadata created from a [`Queued`] track at playback time.
pub struct Decoded {
    /// Has both the formatted name and some information from the decoded data.
    pub info: Info,

    /// The decoded data, which is able to be played by [rodio].
    pub data: DecodedData,
}

impl Decoded {
    /// Creates a new track.
    /// This is equivalent to [`Queued::decode`].
    pub fn new(track: Queued) -> Result<Self> {
        let (path, display) = (track.path.clone(), track.display.clone());
        let data = Decoder::builder()
            .with_byte_len(track.data.len().try_into()?)
            .with_data(Cursor::new(track.data))
            .build()
            .track(track.display)?;

        let info = Info::new(&data, path, display)?;
        Ok(Self { info, data })
    }
}
