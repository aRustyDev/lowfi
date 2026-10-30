//! Shared deterministic audio and loopback HTTP fixtures for tests.

use std::{
    io::{self, Read as _, Write as _},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use bytes::Bytes;
use reqwest::Client;

use crate::tracks::Queued;

/// A four-second, 44.1 kHz mono 440 Hz MP3 generated with:
/// `ffmpeg -f lavfi -i sine=frequency=440:sample_rate=44100:duration=4 -ac 1 -c:a libmp3lame -b:a 64k -write_xing 1 test-tone.mp3`.
pub const TONE_MP3: &[u8] = include_bytes!("fixtures/test-tone.mp3");

/// Builds a queued fixture without downloading; its URL is only a track identifier.
pub fn queued(name: &str) -> Queued {
    Queued::new(
        format!("https://example.invalid/{name}.mp3"),
        Bytes::from_static(TONE_MP3),
        Some(name.to_owned()),
    )
    .unwrap()
}

/// A bounded loopback-test client that ignores environment proxy settings.
/// Fixture requests must stay local even on machines with HTTP proxies configured.
pub fn http_client() -> Client {
    Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(1))
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap()
}

/// Owns one loopback listener and its worker thread.
///
/// Dropping the server cancels pending acceptance and joins the worker, discarding
/// worker failures. Use [`Self::finish`] when the test must inspect I/O errors.
pub struct Server {
    base: String,
    cancelled: Arc<AtomicBool>,
    worker: Option<JoinHandle<io::Result<()>>>,
}

impl Server {
    /// Serves at most one request on an OS-assigned port.
    ///
    /// Acceptance is bounded to five seconds; accepted-stream reads and writes
    /// each have a two-second timeout. The fixture needs no internet service.
    pub fn serve_once(body: &'static [u8]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancelled_for_worker = cancelled.clone();
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if cancelled_for_worker.load(Ordering::Relaxed) {
                    return Ok(());
                }
                match listener.accept() {
                    Ok((stream, _)) => return respond(stream, body),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return Err(io::Error::new(
                                io::ErrorKind::TimedOut,
                                "test client did not connect",
                            ));
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => return Err(error),
                }
            }
        });
        Self {
            base,
            cancelled,
            worker: Some(worker),
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// Cancels pending acceptance, joins the worker, and returns its I/O result.
    ///
    /// Success does not prove a request was served: cancellation before a client
    /// connects also succeeds. Assert the HTTP result separately. Worker panics
    /// propagate here, whereas `Drop` discards both I/O errors and worker panics.
    pub fn finish(mut self) -> io::Result<()> {
        self.cancelled.store(true, Ordering::Relaxed);
        self.worker.take().unwrap().join().unwrap()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn respond(mut stream: TcpStream, body: &[u8]) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut request = Vec::with_capacity(1024);
    let mut chunk = [0; 512];
    while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "incomplete request",
            ));
        }
        request.extend_from_slice(&chunk[..read]);
        if request.len() > 8 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request headers too large",
            ));
        }
    }
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: audio/mpeg\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()
}
