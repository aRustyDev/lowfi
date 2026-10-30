//! An extremely simple lofi player.

use clap::{CommandFactory, Parser, Subcommand};
use std::path::PathBuf;

mod app;
pub mod audio;
pub mod bookmark;
pub mod config;
pub mod downloader;
pub mod error;
#[cfg(all(feature = "menubar", target_os = "macos"))]
mod menubar;
pub mod message;
pub mod player;
#[cfg(feature = "scrape")]
mod scrapers;
pub mod tasks;
mod tests;
pub mod tracks;
pub mod ui;
pub mod volume;

#[cfg(feature = "scrape")]
use crate::scrapers::Source;
pub use error::{Error, Result};
pub use message::Message;
pub use tasks::Tasks;

/// An extremely simple lofi player.
#[derive(Parser, Clone)]
#[command(about, version)]
#[allow(clippy::struct_excessive_bools)]
pub struct Args {
    /// Use an alternate terminal screen.
    #[clap(long, short)]
    alternate: bool,

    /// Hide the bottom control bar.
    #[clap(long, short)]
    minimalist: bool,

    /// Exclude window borders.
    #[clap(long, short)]
    borderless: bool,

    /// Include a clock.
    #[clap(long, short)]
    clock: bool,

    /// Start lowfi paused.
    #[clap(long, short)]
    paused: bool,

    /// FPS of the UI.
    #[clap(long, short, default_value_t = 12)]
    fps: u8,

    /// Timeout in seconds for music downloads.
    #[clap(long, default_value_t = 16)]
    timeout: u64,

    /// Include ALSA & other logs.
    #[clap(long, short)]
    debug: bool,

    /// Width of the player, capped at 32.
    #[clap(long, short, default_value_t = 3)]
    width: usize,

    /// Track list to play music from
    #[clap(long, short, alias = "list", alias = "tracks", short_alias = 'l', default_value_t = String::from("chillhop"))]
    track_list: String,

    /// Internal song buffer size.
    #[clap(long, short = 's', alias = "buffer", default_value_t = 5, value_parser = clap::value_parser!(u32).range(2..))]
    buffer_size: u32,

    /// The command that was ran.
    /// This is [None] if no command was specified.
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Defines all of the extra commands lowfi can run.
#[derive(Subcommand, Clone)]
enum Commands {
    /// Scrapes a music source for files.
    #[cfg(feature = "scrape")]
    Scrape {
        // The source to scrape from.
        source: scrapers::Source,
    },
}

/// Shorthand to get a boolean environment variable.
#[inline]
pub fn env(name: &str) -> bool {
    std::env::var(name).is_ok_and(|x| x == "1")
}

/// Returns the application data directory used for persistency.
///
/// The function returns the platform-specific user data directory with
/// a `lowfi` subfolder. Callers may use this path to store config,
/// bookmarks, and other persistent files.
#[inline]
pub fn data_dir() -> crate::Result<PathBuf> {
    let dir = dirs::data_dir().unwrap().join("lowfi");

    Ok(dir)
}

/// Program entry point.
///
/// Parses CLI arguments, initializes the audio stream and player, then
/// runs the main event loop. On exit it performs cleanup of the UI and
/// returns the inner result.
fn main() -> eyre::Result<()> {
    let args = Args::parse();

    #[cfg(feature = "scrape")]
    if let Some(command) = &args.command {
        return runtime()?.block_on(async {
            match command {
                Commands::Scrape { source } => match source {
                    Source::Lofigirl => scrapers::lofigirl::scrape().await,
                    Source::Chillhop => scrapers::chillhop::scrape().await,
                },
            }
        });
    }

    let config_path = data_dir()?.join("config.toml");
    let overrides = config::CliOverrides::from_args(&args, &Args::command().get_matches());
    let (settings, bindings) = config::load(&config_path, overrides)?;
    let terminal = settings.interfaces.mode.terminal() && !env("LOWFI_DISABLE_UI");

    #[cfg(all(feature = "menubar", target_os = "macos"))]
    if settings.interfaces.mode.menubar() {
        return menubar::run(settings, bindings, terminal).map_err(Into::into);
    }
    if settings.interfaces.mode.menubar() {
        return Err(Error::MenubarUnavailable.into());
    }
    if !terminal {
        terminal_enabled(true)?;
    }

    runtime()?.block_on(run(settings, bindings, terminal))?;
    Ok(())
}

fn runtime() -> eyre::Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?)
}

/// Runs the shared player lifecycle for terminal and native frontends.
async fn run(
    settings: config::Settings,
    bindings: config::BindingMap,
    terminal: bool,
) -> Result<()> {
    let stream = audio::stream()?;
    let environment = terminal
        .then(|| ui::Environment::ready(&settings))
        .transpose()?;
    let mut application =
        match app::Application::init(settings, bindings, stream.mixer(), terminal).await {
            Ok(player) => player,
            Err(error) => {
                if let Some(environment) = environment {
                    environment.cleanup(false)?;
                }
                return Err(error);
            }
        };

    let result = application.run().await;

    if let Some(environment) = environment {
        environment.cleanup(result.is_ok())?;
    }
    application.close().await?;
    result
}

fn terminal_enabled(disabled: bool) -> Result<bool> {
    if disabled && !cfg!(feature = "mpris") {
        return Err(ui::Error::RejectedDisable.into());
    }
    Ok(!disabled)
}

#[cfg(test)]
mod lifecycle_tests {
    #[test]
    fn terminal_disable_preserves_mpris_only_mode() {
        assert!(super::terminal_enabled(false).unwrap());
        if cfg!(feature = "mpris") {
            assert!(!super::terminal_enabled(true).unwrap());
        } else {
            assert!(super::terminal_enabled(true).is_err());
        }
    }
}
