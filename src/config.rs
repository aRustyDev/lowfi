//! Loading and validating Lowfi's user configuration.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crossterm::event::KeyCode;
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

use crate::Message;

/// Fully resolved application settings.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub playback: Playback,
    pub ui: Ui,
    pub bindings: Bindings,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Playback {
    pub paused: bool,
    pub track_list: String,
    pub buffer_size: u32,
    pub timeout: u64,
    pub debug: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Ui {
    pub alternate: bool,
    pub minimalist: bool,
    pub borderless: bool,
    pub clock: bool,
    pub fps: u8,
    pub width: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bindings {
    pub play_pause: Vec<String>,
    pub next: Vec<String>,
    pub quit: Vec<String>,
    pub bookmark: Vec<String>,
    pub volume_up: Vec<String>,
    pub volume_down: Vec<String>,
    pub volume_up_fine: Vec<String>,
    pub volume_down_fine: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            playback: Playback {
                paused: false,
                track_list: "chillhop".into(),
                buffer_size: 5,
                timeout: 16,
                debug: false,
            },
            ui: Ui {
                alternate: false,
                minimalist: false,
                borderless: false,
                clock: false,
                fps: 12,
                width: 3,
            },
            bindings: Bindings {
                play_pause: vec!["p".into(), "Space".into()],
                next: vec!["s".into(), "n".into(), "l".into()],
                quit: vec!["q".into()],
                bookmark: vec!["b".into()],
                volume_up: vec!["+".into(), "=".into(), "k".into(), "Up".into()],
                volume_down: vec!["-".into(), "_".into(), "j".into(), "Down".into()],
                volume_up_fine: vec!["Right".into()],
                volume_down_fine: vec!["Left".into()],
            },
        }
    }
}

/// CLI values that were explicitly supplied, excluding Clap defaults.
#[derive(Default, Serialize)]
pub struct CliOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    playback: Option<PlaybackOverrides>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ui: Option<UiOverrides>,
}

#[derive(Default, Serialize)]
struct PlaybackOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    paused: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    track_list: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    buffer_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    debug: Option<bool>,
}

#[derive(Default, Serialize)]
struct UiOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    alternate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    minimalist: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    borderless: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    clock: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fps: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<usize>,
}

impl CliOverrides {
    pub fn from_args(args: &crate::Args, matches: &clap::ArgMatches) -> Self {
        use clap::parser::ValueSource;
        let supplied = |name| matches.value_source(name) == Some(ValueSource::CommandLine);

        let playback = PlaybackOverrides {
            paused: supplied("paused").then_some(args.paused),
            track_list: supplied("track_list").then(|| args.track_list.clone()),
            buffer_size: supplied("buffer_size").then_some(args.buffer_size),
            timeout: supplied("timeout").then_some(args.timeout),
            debug: supplied("debug").then_some(args.debug),
        };
        let ui = UiOverrides {
            alternate: supplied("alternate").then_some(args.alternate),
            minimalist: supplied("minimalist").then_some(args.minimalist),
            borderless: supplied("borderless").then_some(args.borderless),
            clock: supplied("clock").then_some(args.clock),
            fps: supplied("fps").then_some(args.fps),
            width: supplied("width").then_some(args.width),
        };

        Self {
            playback: (playback.paused.is_some()
                || playback.track_list.is_some()
                || playback.buffer_size.is_some()
                || playback.timeout.is_some()
                || playback.debug.is_some())
            .then_some(playback),
            ui: (ui.alternate.is_some()
                || ui.minimalist.is_some()
                || ui.borderless.is_some()
                || ui.clock.is_some()
                || ui.fps.is_some()
                || ui.width.is_some())
            .then_some(ui),
        }
    }
}

/// Maps terminal keys to their configured messages.
#[derive(Clone, Debug)]
pub struct BindingMap(HashMap<KeyCode, Message>);

impl BindingMap {
    pub fn message(&self, key: &KeyCode) -> Option<Message> {
        self.0.get(key).copied()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read configuration {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid configuration: {0}")]
    Figment(#[from] figment::Error),
    #[error("invalid configuration field `{field}` from {origin}: {reason}")]
    Invalid {
        field: String,
        origin: String,
        reason: String,
    },
}

/// Loads settings from defaults, an optional exact TOML file, environment, and CLI overrides.
pub fn load(path: &Path, cli: CliOverrides) -> Result<(Settings, BindingMap), Error> {
    let exists = path.try_exists().map_err(|source| Error::Read {
        path: path.into(),
        source,
    })?;
    if exists && !path.is_file() {
        return Err(Error::Read {
            path: path.into(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a regular file"),
        });
    }

    let mut figment = Figment::from(Serialized::defaults(Settings::default()));
    if exists {
        figment = figment.merge(Toml::file_exact(path));
    }
    let figment = figment
        .merge(
            Env::prefixed("LOWFI_")
                .ignore(&["disable_ui", "fixed_mpris_name"])
                .split("__"),
        )
        .merge(Serialized::defaults(cli));
    let settings: Settings = figment.extract()?;

    validate_settings(&settings, &figment, path)?;
    let bindings = BindingMap::compile(&settings.bindings, &figment, path)?;
    Ok((settings, bindings))
}

fn validate_settings(settings: &Settings, figment: &Figment, path: &Path) -> Result<(), Error> {
    if settings.ui.fps == 0 {
        return Err(invalid("ui.fps", "must be at least 1", figment, path));
    }
    if settings.playback.buffer_size < 2 {
        return Err(invalid(
            "playback.buffer_size",
            "must be at least 2",
            figment,
            path,
        ));
    }
    Ok(())
}

impl BindingMap {
    fn compile(bindings: &Bindings, figment: &Figment, path: &Path) -> Result<Self, Error> {
        let mut keys = HashMap::new();
        let groups = [
            ("play_pause", &bindings.play_pause, Message::PlayPause),
            ("next", &bindings.next, Message::Next),
            ("quit", &bindings.quit, Message::Quit),
            ("bookmark", &bindings.bookmark, Message::Bookmark),
            ("volume_up", &bindings.volume_up, Message::ChangeVolume(0.1)),
            (
                "volume_down",
                &bindings.volume_down,
                Message::ChangeVolume(-0.1),
            ),
            (
                "volume_up_fine",
                &bindings.volume_up_fine,
                Message::ChangeVolume(0.01),
            ),
            (
                "volume_down_fine",
                &bindings.volume_down_fine,
                Message::ChangeVolume(-0.01),
            ),
        ];

        for (action, values, message) in groups {
            for value in values {
                let key = parse_key(value).map_err(|reason| {
                    invalid(&format!("bindings.{action}"), reason, figment, path)
                })?;
                if let Some(existing) = keys.insert(key.clone(), message) {
                    let existing = action_for(existing);
                    return Err(Error::Invalid {
                        field: format!("bindings.{action}"),
                        origin: source_for(&format!("bindings.{action}"), figment, path),
                        reason: format!("duplicates `{value}` already assigned to `bindings.{existing}` from {}", source_for(&format!("bindings.{existing}"), figment, path)),
                    });
                }
            }
        }
        Ok(Self(keys))
    }
}

fn parse_key(value: &str) -> Result<KeyCode, String> {
    match value.to_ascii_lowercase().as_str() {
        "space" => Ok(KeyCode::Char(' ')),
        "up" => Ok(KeyCode::Up),
        "down" => Ok(KeyCode::Down),
        "left" => Ok(KeyCode::Left),
        "right" => Ok(KeyCode::Right),
        _ if value.chars().count() == 1 => {
            let character = value.chars().next().unwrap();
            if character.is_control() {
                Err("control keys and modifier chords are reserved".into())
            } else {
                Ok(KeyCode::Char(character.to_ascii_lowercase()))
            }
        }
        _ => Err("expected one printable character, Space, Up, Down, Left, or Right".into()),
    }
}

fn action_for(message: Message) -> &'static str {
    match message {
        Message::PlayPause => "play_pause",
        Message::Next => "next",
        Message::Quit => "quit",
        Message::Bookmark => "bookmark",
        Message::ChangeVolume(change) if change > 0.09 => "volume_up",
        Message::ChangeVolume(change) if change < -0.09 => "volume_down",
        Message::ChangeVolume(change) if change > 0.0 => "volume_up_fine",
        Message::ChangeVolume(_) => "volume_down_fine",
        _ => unreachable!("only terminal binding messages are compiled"),
    }
}

fn invalid(field: &str, reason: impl Into<String>, figment: &Figment, path: &Path) -> Error {
    Error::Invalid {
        field: field.into(),
        origin: source_for(field, figment, path),
        reason: reason.into(),
    }
}

fn source_for(field: &str, figment: &Figment, path: &Path) -> String {
    let metadata = figment.find_metadata(field);
    match metadata.map(|metadata| metadata.name.as_ref()) {
        Some(name) if name.contains("environment") => {
            format!("LOWFI_{}", field.replace('.', "__").to_ascii_uppercase())
        }
        Some(name) if name.contains("TOML") => path.display().to_string(),
        Some(name) if name.contains("CliOverrides") => format!(
            "CLI flag --{}",
            field.rsplit('.').next().unwrap().replace('_', "-")
        ),
        _ => "built-in defaults".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};
    use figment::Jail;

    #[test]
    fn defaults_preserve_every_existing_binding() {
        figment::Jail::expect_with(|_| {
            let path = Path::new("missing.toml");
            let (settings, keys) = load(path, CliOverrides::default()).unwrap();
            assert_eq!(settings, Settings::default());
            assert!(!path.exists());
            for (key, message) in [
                ("p", Message::PlayPause),
                (" ", Message::PlayPause),
                ("s", Message::Next),
                ("n", Message::Next),
                ("l", Message::Next),
                ("q", Message::Quit),
                ("b", Message::Bookmark),
                ("+", Message::ChangeVolume(0.1)),
                ("=", Message::ChangeVolume(0.1)),
                ("k", Message::ChangeVolume(0.1)),
                ("-", Message::ChangeVolume(-0.1)),
                ("_", Message::ChangeVolume(-0.1)),
                ("j", Message::ChangeVolume(-0.1)),
                ("Up", Message::ChangeVolume(0.1)),
                ("Down", Message::ChangeVolume(-0.1)),
                ("Right", Message::ChangeVolume(0.01)),
                ("Left", Message::ChangeVolume(-0.01)),
            ] {
                assert_eq!(keys.message(&parse_key(key).unwrap()), Some(message));
            }

            Ok(())
        });
    }

    #[test]
    fn file_environment_and_cli_precedence() {
        Jail::expect_with(|jail| {
            jail.create_file("config.toml", "[ui]\nfps = 20\n")?;
            assert_eq!(
                load(Path::new("config.toml"), CliOverrides::default())
                    .unwrap()
                    .0
                    .ui
                    .fps,
                20
            );
            jail.set_env("LOWFI_UI__FPS", "30");
            assert_eq!(
                load(Path::new("config.toml"), CliOverrides::default())
                    .unwrap()
                    .0
                    .ui
                    .fps,
                30
            );
            let cli = CliOverrides {
                ui: Some(UiOverrides {
                    fps: Some(40),
                    ..UiOverrides::default()
                }),
                ..CliOverrides::default()
            };
            assert_eq!(load(Path::new("config.toml"), cli).unwrap().0.ui.fps, 40);
            Ok(())
        });
    }

    #[test]
    fn invalid_and_duplicate_bindings_are_rejected() {
        Jail::expect_with(|jail| {
            jail.create_file("config.toml", "[bindings]\nnext = [\"q\"]\n")?;
            let error = load(Path::new("config.toml"), CliOverrides::default())
                .unwrap_err()
                .to_string();
            assert!(error.contains("duplicates"));
            assert!(error.contains("config.toml"));
            Ok(())
        });
    }

    #[test]
    fn file_overrides_replace_individual_binding_lists() {
        Jail::expect_with(|jail| {
            jail.create_file("config.toml", "[bindings]\nnext = [\"x\"]\nbookmark = []\n")?;
            let (_, keys) = load(Path::new("config.toml"), CliOverrides::default()).unwrap();
            assert_eq!(keys.message(&parse_key("x").unwrap()), Some(Message::Next));
            assert_eq!(keys.message(&parse_key("s").unwrap()), None);
            assert_eq!(keys.message(&parse_key("b").unwrap()), None);
            Ok(())
        });
    }

    #[test]
    fn invalid_values_identify_their_source() {
        Jail::expect_with(|jail| {
            jail.create_file("config.toml", "[ui]\nfps = 0\n")?;
            let error = load(Path::new("config.toml"), CliOverrides::default())
                .unwrap_err()
                .to_string();
            assert!(error.contains("ui.fps"));
            assert!(error.contains("config.toml"));

            jail.create_file("small-buffer.toml", "[playback]\nbuffer_size = 1\n")?;
            let error = load(Path::new("small-buffer.toml"), CliOverrides::default())
                .unwrap_err()
                .to_string();
            assert!(error.contains("playback.buffer_size"));
            assert!(error.contains("small-buffer.toml"));

            jail.set_env("LOWFI_PLAYBACK__BUFFER_SIZE", "1");
            let error = load(Path::new("missing.toml"), CliOverrides::default())
                .unwrap_err()
                .to_string();
            assert!(error.contains("LOWFI_PLAYBACK__BUFFER_SIZE"));
            Ok(())
        });
    }

    #[test]
    fn malformed_unknown_and_reserved_bindings_fail() {
        Jail::expect_with(|jail| {
            jail.create_file("bad.toml", "[ui\nfps = 10")?;
            assert!(load(Path::new("bad.toml"), CliOverrides::default()).is_err());

            jail.create_file("unknown.toml", "[ui]\nnot_a_setting = true")?;
            assert!(load(Path::new("unknown.toml"), CliOverrides::default()).is_err());

            jail.create_file("reserved.toml", "[bindings]\nquit = [\"Ctrl+C\"]")?;
            assert!(load(Path::new("reserved.toml"), CliOverrides::default()).is_err());
            Ok(())
        });
    }

    #[test]
    fn only_explicit_cli_values_override_configuration() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "config.toml",
                "[playback]\npaused = false\ntrack_list = \"from-file\"\nbuffer_size = 2\ntimeout = 1\ndebug = false\n\n[ui]\nalternate = false\nminimalist = false\nborderless = false\nclock = false\nfps = 7\nwidth = 1\n",
            )?;

            let args = crate::Args::try_parse_from(["lowfi"]).unwrap();
            let matches = crate::Args::command()
                .try_get_matches_from(["lowfi"])
                .unwrap();
            let overrides = CliOverrides::from_args(&args, &matches);
            assert_eq!(
                load(Path::new("config.toml"), overrides).unwrap().0.ui.fps,
                7
            );

            let load_cli = |arguments: &[&str]| {
                let args = crate::Args::try_parse_from(arguments).unwrap();
                let matches = crate::Args::command()
                    .try_get_matches_from(arguments)
                    .unwrap();
                load(
                    Path::new("config.toml"),
                    CliOverrides::from_args(&args, &matches),
                )
                .unwrap()
                .0
            };

            assert!(load_cli(&["lowfi", "--paused"]).playback.paused);
            assert_eq!(
                load_cli(&["lowfi", "--track-list", "from-cli"])
                    .playback
                    .track_list,
                "from-cli"
            );
            assert_eq!(
                load_cli(&["lowfi", "--buffer-size", "8"])
                    .playback
                    .buffer_size,
                8
            );
            assert_eq!(load_cli(&["lowfi", "--timeout", "42"]).playback.timeout, 42);
            assert!(load_cli(&["lowfi", "--debug"]).playback.debug);
            assert!(load_cli(&["lowfi", "--alternate"]).ui.alternate);
            assert!(load_cli(&["lowfi", "--minimalist"]).ui.minimalist);
            assert!(load_cli(&["lowfi", "--borderless"]).ui.borderless);
            assert!(load_cli(&["lowfi", "--clock"]).ui.clock);
            assert_eq!(load_cli(&["lowfi", "--fps", "17"]).ui.fps, 17);
            assert_eq!(load_cli(&["lowfi", "--width", "9"]).ui.width, 9);
            Ok(())
        });
    }

    #[test]
    fn binding_action_names_cover_every_binding_message() {
        for (message, expected) in [
            (Message::PlayPause, "play_pause"),
            (Message::Next, "next"),
            (Message::Quit, "quit"),
            (Message::Bookmark, "bookmark"),
            (Message::ChangeVolume(0.1), "volume_up"),
            (Message::ChangeVolume(0.09), "volume_up_fine"),
            (Message::ChangeVolume(-0.1), "volume_down"),
            (Message::ChangeVolume(-0.09), "volume_down_fine"),
            (Message::ChangeVolume(0.01), "volume_up_fine"),
            (Message::ChangeVolume(-0.01), "volume_down_fine"),
            (Message::ChangeVolume(0.0), "volume_down_fine"),
        ] {
            assert_eq!(action_for(message), expected);
        }
    }

    #[test]
    fn invalid_cli_values_identify_the_cli_flag_as_the_source() {
        figment::Jail::expect_with(|_| {
            let args = crate::Args::try_parse_from(["lowfi", "--fps", "0"]).unwrap();
            let matches = crate::Args::command()
                .try_get_matches_from(["lowfi", "--fps", "0"])
                .unwrap();
            let error = load(
                Path::new("missing.toml"),
                CliOverrides::from_args(&args, &matches),
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains("CLI flag --fps"));

            Ok(())
        });
    }

    #[test]
    fn built_in_defaults_are_identified_as_the_source() {
        let figment = Figment::from(Serialized::defaults(Settings::default()));
        assert_eq!(
            source_for("ui.fps", &figment, Path::new("missing.toml")),
            "built-in defaults"
        );
    }

    #[test]
    fn legacy_environment_switches_do_not_enter_structured_settings() {
        Jail::expect_with(|jail| {
            jail.set_env("LOWFI_DISABLE_UI", "1");
            jail.set_env("LOWFI_FIXED_MPRIS_NAME", "1");
            let (settings, _) = load(Path::new("missing.toml"), CliOverrides::default()).unwrap();
            assert_eq!(settings, Settings::default());
            jail.set_env("LOWFI_UI__NOT_A_SETTING", "1");
            assert!(load(Path::new("missing.toml"), CliOverrides::default()).is_err());
            Ok(())
        });
    }
}
