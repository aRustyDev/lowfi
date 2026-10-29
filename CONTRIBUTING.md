# Contributing to lowfi

[[Version française](./docs/fr/CONTRIBUER.md)]

There are a few guidelines outlined here that will make it more likely for your PR to be accepted.
Only ones that are less obvious are going to be listed. If you need to ask, it's probably a no.

## 1. AI

You can use AI for searching or if there's something minor and tedious (eg. tests) that you'd like
to avoid having to do manually.

With that said, if it is noticeable that you used AI then it is way too much.
AI generated PR's do not help maintainers, it's just a hassle and frequently wastes their time.

## 2. Smaller is better

Try and make it so that each PR is one contained feature. Adding multiple features in a PR is usually a bad idea.
This is also so that individual features can be approved or denied, rather than that having to be for a more significant
chunk of code.

## 3. Keep lowfi simple

lowfi is supposed simple program. For now, no changes to the initial user-facing UI will be accepted.
The UI of lowfi playing a song has stayed identical since the first versions, since complicating it
detracts from it's purpose.

More complex features, like fancy colors or cover art, will not be accepted ever. Implementations of
acceptable features should also be simple and not too obtrusive. Even if a feature is simple,
if it is very complex to implement, then it won't be accepted.

## Testing

Install the pinned local test tools once:

```sh
cargo install --locked cargo-mutants --version 27.1.0
cargo install --locked cargo-nextest --version 0.9.140
cargo install --locked cargo-feature-combinations --version 0.7.0
```

Linux contributors need the ALSA development headers (for example,
`libasound2-dev` on Ubuntu). The standard fallback remains `cargo test`;
nextest does not silently fall back when it fails.

```sh
cargo fmt --check
cargo test --locked
cargo nextest run --locked
cargo nextest run --locked --no-default-features
cargo fc matrix --pretty
cargo fc check --locked --all-targets
cargo mutants --list
cargo mutants
cargo mutants --test-tool cargo
```

Run platform integration features only on their native platform:

```sh
# Linux
cargo nextest run --locked --features mpris,scrape,extra-audio-formats

# Windows
cargo nextest run --locked --features media-controls,scrape,extra-audio-formats

# macOS
cargo nextest run --locked --features menubar,macos-app,scrape,extra-audio-formats

# Native mutation passes
# Linux core:    cargo mutants --config .cargo/mutants.toml --features default
# Linux MPRIS:   cargo mutants --config .cargo/mutants-mpris.toml --features mpris
# Windows core:  cargo mutants --config .cargo/mutants.toml --features default
# Windows SMTC:  cargo mutants --config .cargo/mutants-media-controls.toml --features media-controls
# macOS core:    cargo mutants --config .cargo/mutants.toml --features menubar,macos-app
```

The curated `cargo fc` matrix includes empty, default, scraping, extra-audio,
and native feature sets for the current platform. It deliberately does not
build macOS-only features on Linux or Windows-only media controls on other
platforms.

Mutation testing uses three explicit scopes. The default configuration covers
`src/player.rs`, `src/config.rs`, and terminal input mapping; `input::listen`
is excluded with `#[cfg_attr(test, mutants::skip)]` because it owns the terminal
event loop. The Linux MPRIS
configuration covers its in-process `Player` and `Sender`; all `Server`
construction, dispatch, signal/property publication, and D-Bus handling are
excluded because they require a session bus. The Windows configuration covers
only `Server::map_event`; native control construction, synchronization
(`sync_playback` and `sync_metadata`), teardown, window procedures, and
message pumping require Windows SMTC. `sync_metadata` carries
`#[cfg_attr(test, mutants::skip)]` because cargo-mutants does not apply regex
exclusions to struct-field mutants; the attribute keeps its metadata-publication
fields out of this in-process scope.
Packaging launchers, generated files, fixtures, and test code are also outside
the mutation targets. Native CI compiles those feature gates.

The checked-in fixture at `src/tests/fixtures/test-tone.mp3` is a four-second,
mono 44.1 kHz MP3. Its README documents the FFmpeg generation command and the
three-to-five-second decoder tolerance. Tests serve it from a bounded localhost
fixture; no internet service or audio device is needed.

Treat every surviving mutant as a test gap unless this documented boundary
explains it. Run an inventory before a mutation pass and inspect its report:

```sh
cargo mutants --config .cargo/mutants.toml --list
cargo mutants --config .cargo/mutants-mpris.toml --features mpris --list
cargo mutants --config .cargo/mutants-media-controls.toml --features media-controls --list
cargo mutants --config .cargo/mutants.toml --re '^src/config\.rs:' --list
cargo mutants --config .cargo/mutants.toml --re '^src/player\.rs:' --list
```

`--re` narrows the configured target list, so these focused inventories contain
only the requested file. After a run, inspect `mutants.out/missed.txt`,
`mutants.out/timeout.txt`, `mutants.out/unviable.txt`, and
`mutants.out/outcomes.json`. A timeout or execution error needs investigation;
do not treat it as an exclusion. Use `cargo mutants --test-tool cargo` only
when nextest itself is unavailable, and do not retry a failed nextest run with
another runner automatically.
