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

Install the pinned local tools:

```sh
cargo install --locked cargo-nextest --version 0.9.140
cargo install --locked cargo-feature-combinations --version 0.7.0
cargo install --locked cargo-mutants --version 27.1.0
```

These versions match CI. `--version` selects the tool release; `--locked` uses
that release's packaged dependency lockfile. New releases are not installed
automatically. See the [Cargo installation reference](https://doc.rust-lang.org/cargo/commands/cargo-install.html#dealing-with-the-lockfile).

To evaluate a newer release locally, replace its `--version` value above and
keep `--locked`. Review the release notes and required Rust version; test tools
may require a newer compiler than Lowfi itself. When adopting the release,
update the version here and wherever that tool is installed in
`.github/workflows/tooling.yml` and `.github/workflows/mutation.yml` together.
Verify default/no-default Nextest runs, curated feature checks, and mutation
inventories on the affected native platforms. Run targeted mutation passes if
the upgrade changes mutation behavior. Commit the version updates and validation
results together so contributors and CI use the same tested baseline.

Run `cargo fmt --check`, `cargo test --locked`, `cargo nextest run --locked`,
`cargo nextest run --locked --no-default-features`, and
`cargo fc check --locked --all-targets`. Linux needs ALSA development headers
(`libasound2-dev` on Ubuntu). The curated matrix includes only native platform
features: MPRIS on Linux and media controls on Windows. macOS uses the common
scraping and extra-audio features.

The checked-in four-second MP3 and bounded localhost fixture make download and
decode tests independent of internet services and audio devices. Its generation
command and decoder tolerance are documented beside the fixture.

Mutation inventories and passes are explicit:

```sh
cargo mutants --list
cargo mutants
# Linux only
cargo mutants --config .cargo/mutants-mpris.toml --features mpris --list
cargo mutants --config .cargo/mutants-mpris.toml --features mpris
# Windows only
cargo mutants --config .cargo/mutants-media-controls.toml --features media-controls --list
cargo mutants --config .cargo/mutants-media-controls.toml --features media-controls
```

The default mutation scope covers configuration and terminal input mapping.
The terminal event loop is excluded with `mutants::skip`. The Linux scope covers
in-process MPRIS Player and Sender behavior; Server construction, D-Bus traffic,
and native property publication are excluded. The Windows scope covers event
mapping; native control construction, publication, teardown, and message pumping
are excluded. `sync_metadata` uses `mutants::skip` because struct-field mutants
cannot be excluded reliably by function-name regex. Packaging, fixtures, and
tests are outside mutation scopes.

Treat surviving mutants as test gaps unless these boundaries explain them.
Inspect `mutants.out/missed.txt`, `timeout.txt`, `unviable.txt`, and
`outcomes.json`; timeouts and execution errors require investigation. Use
`cargo mutants --test-tool cargo` only when nextest is unavailable, and never
retry a failed nextest run automatically with another runner.
