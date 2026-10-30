# Configuration

Lowfi reads settings once at startup. You can combine an optional TOML file,
environment variables, and command-line flags. Restart Lowfi after editing the
file; settings are not reloaded while it runs.

## File location and quick start

Create `config.toml` in Lowfi's platform data directory:

| Platform | Default path |
| --- | --- |
| Linux | `~/.local/share/lowfi/config.toml` |
| macOS | `~/Library/Application Support/lowfi/config.toml` |
| Windows | `%APPDATA%\lowfi\config.toml` (Roaming AppData) |

On Linux, an absolute `XDG_DATA_HOME` overrides `~/.local/share`, so the path
becomes `$XDG_DATA_HOME/lowfi/config.toml`. Lowfi uses its **data** directory;
`XDG_CONFIG_HOME` does not select this file.

Create the parent directory if needed, then save only the settings you want to
change. For example:

```toml
[playback]
paused = true

[ui]
fps = 20

[bindings]
next = ["x"]
```

This starts paused, renders at 20 FPS, and replaces the configurable Next keys
with `x`. Omitted fields keep their built-in defaults. A missing file is valid;
Lowfi never creates or overwrites it. Existing volume and bookmark persistence
is separate from this configuration file.

## Precedence

Later sources override earlier sources, one setting at a time:

1. Built-in defaults
2. `config.toml`
3. `LOWFI_*` environment variables
4. Explicit command-line flags

For example, a file with `ui.fps = 20` is overridden by `LOWFI_UI__FPS=24`,
and `--fps 30` overrides both. Omitting `--fps` preserves the file or environment
value; the CLI's displayed default does not override it.

Boolean flags such as `--paused` set their option to `true`. To override a
file's `paused = true` with `false`, use `LOWFI_PLAYBACK__PAUSED=false`.
These flags do not accept `--paused=false` or have a `--no-paused` counterpart.

Environment names use `LOWFI_<SECTION>__<FIELD>` with a double underscore
between the section and field. Use `true` or `false` for structured booleans
and array syntax for bindings. For example, in a POSIX shell:

```sh
LOWFI_PLAYBACK__PAUSED=false LOWFI_UI__FPS=24 lowfi --fps 30
LOWFI_BINDINGS__NEXT='["x", "n"]' lowfi
```

In PowerShell:

```powershell
$env:LOWFI_PLAYBACK__PAUSED = 'false'
$env:LOWFI_BINDINGS__NEXT = '["x", "n"]'
lowfi
```

The legacy `LOWFI_DISABLE_UI` and `LOWFI_FIXED_MPRIS_NAME` switches still use
`1` to enable them. They are handled separately from these structured settings;
see [Environment Variables](ENVIRONMENT_VARS.md).

## Playback options

These fields belong under `[playback]` in TOML.

| Key | Purpose | Default / accepted values | Environment variable | CLI flag |
| --- | --- | --- | --- | --- |
| `paused` | Start playback paused | `false`; boolean | `LOWFI_PLAYBACK__PAUSED` | `-p`, `--paused` |
| `track_list` | Track list to play | `"chillhop"`; string | `LOWFI_PLAYBACK__TRACK_LIST` | `-t`, `--track-list` |
| `buffer_size` | Internal song buffer size | `5`; integer from 2 to 4294967295 | `LOWFI_PLAYBACK__BUFFER_SIZE` | `-s`, `--buffer-size` |
| `timeout` | Download timeout in seconds | `16`; nonnegative 64-bit integer | `LOWFI_PLAYBACK__TIMEOUT` | `--timeout` |
| `debug` | Include ALSA and downloader debug logs | `false`; boolean | `LOWFI_PLAYBACK__DEBUG` | `-d`, `--debug` |

See [Custom track lists](../README.md#custom-track-lists) for track-list names
and file formats. Volume is stored separately and has no `[playback]` setting.

## Terminal options

These fields belong under `[ui]` in TOML.

| Key | Purpose | Default / accepted values | Environment variable | CLI flag |
| --- | --- | --- | --- | --- |
| `alternate` | Use the alternate terminal screen | `false`; boolean | `LOWFI_UI__ALTERNATE` | `-a`, `--alternate` |
| `minimalist` | Hide the bottom control bar | `false`; boolean | `LOWFI_UI__MINIMALIST` | `-m`, `--minimalist` |
| `borderless` | Hide window borders | `false`; boolean | `LOWFI_UI__BORDERLESS` | `-b`, `--borderless` |
| `clock` | Show a clock | `false`; boolean | `LOWFI_UI__CLOCK` | `-c`, `--clock` |
| `fps` | Terminal frames per second | `12`; integer from 1 to 255 | `LOWFI_UI__FPS` | `-f`, `--fps` |
| `width` | Player width parameter | `3`; nonnegative platform-sized integer, capped at 32 for rendering | `LOWFI_UI__WIDTH` | `-w`, `--width` |

`width` is a sizing parameter, not the final width in terminal columns. Values
above 32 are accepted and render identically to 32, matching the existing CLI
behavior. Negative values and values that do not fit the platform's unsigned
integer type are rejected.

## Key bindings

Each field under `[bindings]` is an array of key names. Its environment variable
is `LOWFI_BINDINGS__<FIELD>`, for example `LOWFI_BINDINGS__PLAY_PAUSE`.
Bindings have no CLI flags.

| Key | Action | Default keys |
| --- | --- | --- |
| `play_pause` | Toggle playback | `["p", "Space"]` |
| `next` | Skip to the next track | `["s", "n", "l"]` |
| `quit` | Quit Lowfi | `["q"]` |
| `bookmark` | Toggle the current track's bookmark | `["b"]` |
| `volume_up` | Raise volume by 10 percentage points | `["+", "=", "k", "Up"]` |
| `volume_down` | Lower volume by 10 percentage points | `["-", "_", "j", "Down"]` |
| `volume_up_fine` | Raise volume by 1 percentage point | `["Right"]` |
| `volume_down_fine` | Lower volume by 1 percentage point | `["Left"]` |

A list replaces that action's entire default list; it does not append keys.
Other actions keep their defaults. An empty list disables that action's
configurable keys:

```toml
[bindings]
next = ["x"]
bookmark = []
```

Key names accept one printable character or `Space`, `Up`, `Down`, `Left`, and
`Right`. ASCII letter keys and named keys are case-insensitive. Modifier chords
such as `Ctrl+X` are not supported. `Ctrl+C` always quits and cannot be remapped.
Duplicate keys, including duplicates within one action, are rejected. To move a
key between actions, remove it from its original action's list as well.

Terminal media-key handling is separate from these lists. Disabling an action's
configurable keys does not disable its media-key handling. The terminal control
bar continues to show the default hints; custom bindings do not change its text.

## Validation and troubleshooting

Lowfi stops startup for an unreadable or malformed file, unknown fields,
incorrect types, invalid values, or invalid/duplicate bindings. Errors identify
the setting and its source where available: a file path, environment variable,
CLI flag, or built-in default. For example, `ui.fps = 0` fails validation and
identifies the configuration file.

If a file change appears to have no effect, restart Lowfi and check for an
environment variable or explicit CLI flag overriding that setting. Check the
data-directory path above, especially `XDG_DATA_HOME` on Linux. Shells differ in
how they quote array values; preserve the double quotes inside the binding
array as shown in the examples.
