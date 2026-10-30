# Lowfi upstream PR reconstruction

Local audit dated 2026-10-29. This document belongs only to `stack/integration`; it is excluded from all eight PR branches. No branches were pushed and no PRs were opened.

## Result and branch topology

Pinned upstream: `talwat/lowfi:main` at `d86a51e93375efcfa10b5b98878113989e1b673f`. Integration code was validated at `eb78506b371b34132d2e3e0b495d26d0b0401c45`; the subsequent audit commit changes documentation only.

| Branch | Parent | Tip | Default Cargo tests |
| --- | --- | --- | --- |
| `stack/0/agents` | `upstream/main` | `fb3eda3` | 33 passed |
| `stack/0/ci` | `upstream/main` | `e98cb78` | 33 passed |
| `stack/0/figment` | `upstream/main` | `c57e462` | 46 passed |
| `stack/0/nextest` | `stack/0/figment` | `44afeef` | 46 passed |
| `stack/1/decouple-playback-core` | `stack/0/nextest` | `2c39a6b` | 55 passed |
| `stack/2/bounded-playback-history` | `stack/0/nextest` | `75c7dd3` | 60 passed |
| `stack/3/macos-menubar` | `stack/1/decouple-playback-core` | `47c0b10` | 59 passed |
| `stack/4/macos-app-pkg` | `stack/0/nextest` | `54a4d98` | 46 passed |

All declared parents and integration ancestry were checked. History and packaging have no core dependency; menubar depends on core. AGENTS and CI have no Figment dependency. New branches have `pushRemote = origin` and no accidental tracking relationship to upstream main. Existing branch configuration remains unchanged.

The original history commit `234c75f` is empty and shares its tree with `cde74c6`. Features were reconstructed from source and behavior rather than blindly cherry-picked. Original `merged` and `origin/merged` have identical trees despite different ancestry.

## Source and worktree preservation

All seven original branch refs remain at their captured values. All four original worktree statuses and all four dirty/untracked file hashes match the pre-reconstruction snapshot. Only the new managed worktree was edited.

| Original branch | Preserved tip |
| --- | --- |
| `codex/bounded-playback-history` | `234c75fc22bcf9bf8f5c3fdf110f5e011b70348e` |
| `codex/decouple-playback-core` | `cde74c6f07368efb8a4255d8b8acdc534b64dbc0` |
| `codex/macos-app-pkg` | `30923bba62bef8613dd0bdc5a8f79bb2d8a4e203` |
| `codex/macos-menubar` | `75b1d61fe4a2f45d171443af7023fb78ae704131` |
| `codex/testing-hardening-base` | `434a3fc620e79554638a04bbac026b271c26b6e3` |
| `main` | `35d95e5f0f85a7c38667c9f50e15a6e6e294943c` |
| `merged` | `71bda9325c3107d5982367930fbbe92bda5c5a6e` |

The lockfile, packaging tree, menu icon assets, application composition, terminal renderer, platform adapter file for Windows, and existing UI/track test files match the original `merged` byte for byte. The verified byte-identical paths are:

- `Cargo.lock`
- `src/app.rs`
- `src/downloader.rs`
- `src/ui.rs`
- `src/ui/init.rs`
- `src/ui/environment.rs`
- `src/ui/interface.rs`
- `src/ui/interface/components.rs`
- `src/ui/interface/window.rs`
- `src/ui/interface/titlebar.rs`
- `src/ui/interface/clock.rs`
- `src/ui/media_controls.rs`
- `src/tests/ui.rs`
- `src/tests/tracks.rs`
- `src/tracks.rs`
- `packaging/macos/Info.plist.in`
- `packaging/macos/Lowfi.command`
- `packaging/macos/Lowfi.icns`
- `packaging/macos/Lowfi.png`
- `packaging/macos/build-app.sh`
- `packaging/macos/launcher.rs`
- `packaging/macos/verify-app.sh`
- `docs/media/menubar.svg`
- `docs/media/menubar-icon.md`

## Change ownership ledger

Every path changed between upstream and the original integration snapshot is assigned below. Shared native formatting/assertion fixes are intentionally present in both the independent CI branch and the testing baseline. Source editor configuration is excluded.

| Original changed path | PR branch ownership |
| --- | --- |
| `.cargo/mutants-media-controls.toml` | `stack/0/nextest` |
| `.cargo/mutants-mpris.toml` | `stack/0/nextest` |
| `.cargo/mutants.toml` | `stack/0/nextest`, `stack/1/decouple-playback-core`, `stack/2/bounded-playback-history` |
| `.github/config/serena/.gitignore` | Excluded: fork-specific Serena configuration |
| `.github/config/serena/project.yml` | Excluded: fork-specific Serena configuration |
| `.github/workflows/mutation.yml` | `stack/0/nextest`, `stack/3/macos-menubar` |
| `.github/workflows/tests.yml` | `stack/0/ci` |
| `.gitignore` | `stack/0/nextest` |
| `AGENTS.md` | `stack/0/agents` |
| `CONTRIBUTING.md` | `stack/0/nextest`, `stack/1/decouple-playback-core`, `stack/2/bounded-playback-history`, `stack/3/macos-menubar`, `stack/4/macos-app-pkg` |
| `Cargo.lock` | `stack/0/figment`, `stack/0/nextest`, `stack/3/macos-menubar` |
| `Cargo.toml` | `stack/0/figment`, `stack/0/nextest`, `stack/3/macos-menubar`, `stack/4/macos-app-pkg` |
| `README.md` | `stack/0/figment`, `stack/1/decouple-playback-core`, `stack/2/bounded-playback-history`, `stack/3/macos-menubar`, `stack/4/macos-app-pkg` |
| `docs/ARCHITECTURE.md` | `stack/1/decouple-playback-core`, `stack/3/macos-menubar` |
| `docs/media/menubar-icon.md` | `stack/3/macos-menubar` |
| `docs/media/menubar.svg` | `stack/3/macos-menubar` |
| `packaging/macos/Info.plist.in` | `stack/4/macos-app-pkg` |
| `packaging/macos/Lowfi.command` | `stack/4/macos-app-pkg` |
| `packaging/macos/Lowfi.icns` | `stack/4/macos-app-pkg` |
| `packaging/macos/Lowfi.png` | `stack/4/macos-app-pkg` |
| `packaging/macos/build-app.sh` | `stack/4/macos-app-pkg` |
| `packaging/macos/launcher.rs` | `stack/4/macos-app-pkg` |
| `packaging/macos/verify-app.sh` | `stack/4/macos-app-pkg` |
| `src/app.rs` | `stack/1/decouple-playback-core`, `stack/3/macos-menubar` |
| `src/audio/waiter.rs` | `stack/1/decouple-playback-core`, `stack/2/bounded-playback-history` |
| `src/config.rs` | `stack/0/figment`, `stack/2/bounded-playback-history`, `stack/3/macos-menubar` |
| `src/downloader.rs` | `stack/0/figment`, `stack/1/decouple-playback-core`, `stack/2/bounded-playback-history` |
| `src/error.rs` | `stack/1/decouple-playback-core`, `stack/3/macos-menubar` |
| `src/main.rs` | `stack/0/figment`, `stack/1/decouple-playback-core`, `stack/2/bounded-playback-history`, `stack/3/macos-menubar` |
| `src/menubar.rs` | `stack/3/macos-menubar` |
| `src/message.rs` | `stack/1/decouple-playback-core`, `stack/2/bounded-playback-history` |
| `src/player.rs` | `stack/0/figment`, `stack/1/decouple-playback-core`, `stack/2/bounded-playback-history`, `stack/3/macos-menubar` |
| `src/scrapers.rs` | `stack/0/ci`, `stack/0/nextest` |
| `src/tests.rs` | `stack/0/nextest` |
| `src/tests/fixture.rs` | `stack/0/nextest` |
| `src/tests/fixtures/README.md` | `stack/0/nextest` |
| `src/tests/fixtures/test-tone.mp3` | `stack/0/nextest` |
| `src/tests/tracks.rs` | `stack/0/nextest` |
| `src/tests/ui.rs` | `stack/0/ci`, `stack/0/nextest`, `stack/1/decouple-playback-core` |
| `src/tracks.rs` | `stack/2/bounded-playback-history` |
| `src/ui.rs` | `stack/1/decouple-playback-core`, `stack/2/bounded-playback-history` |
| `src/ui/environment.rs` | `stack/0/figment` |
| `src/ui/init.rs` | `stack/0/ci`, `stack/0/figment`, `stack/1/decouple-playback-core` |
| `src/ui/input.rs` | `stack/0/figment`, `stack/0/nextest`, `stack/2/bounded-playback-history` |
| `src/ui/interface.rs` | `stack/0/figment`, `stack/1/decouple-playback-core`, `stack/2/bounded-playback-history` |
| `src/ui/interface/components.rs` | `stack/1/decouple-playback-core` |
| `src/ui/media_controls.rs` | `stack/0/ci`, `stack/0/nextest`, `stack/1/decouple-playback-core` |
| `src/ui/mpris.rs` | `stack/0/nextest`, `stack/1/decouple-playback-core`, `stack/2/bounded-playback-history` |

Additional files introduced during the split: `src/history.rs` belongs to history; `.github/workflows/tooling.yml` belongs to Nextest; `.github/workflows/macos-app.yml` belongs to packaging; this audit belongs to integration only. Package metadata, lockfile updates, and CI feature rows are included only where their corresponding features exist.

The original packaging additions to the shared test workflow are represented by the standalone packaging workflow. The original test workflow is split into independent standard CI and testing-tool CI. Mutation scopes follow features; the final default inventory adds the extracted history module. The fragile Windows source-line exclusion was removed while retaining the existing `mutants::skip` boundary.

## Required correctness and compatibility repairs

- Figment ignores the existing `LOWFI_DISABLE_UI` and `LOWFI_FIXED_MPRIS_NAME` switches when extracting structured settings, while still rejecting unknown structured settings. The regression test was observed failing before the fix.
- Configuration-reading tests use Figment Jail consistently, preventing concurrent process-environment changes from affecting unrelated tests.
- Core preserves Linux MPRIS operation with the terminal disabled, and includes a regression test for that decision.
- History emits real generation-tagged completion and restart-threshold messages from the waiter. Entering loading invalidates old notifications; Previous restores retained audio and late Loaded messages are ignored. The loading regression test was observed failing before the fix.
- History exposes configurable `bindings.previous` with default `h`, plus terminal TrackPrevious and Linux MPRIS Previous. The terminal layout and displayed control bar are unchanged.
- The standalone history adapter uses Rodio getters directly; the composed adapter uses the core playback view. Windows SMTC Previous remains unavailable.
- Menubar initializes and cleans up the terminal when terminal mode is active and closes its native loop when its worker stops.
- README no longer claims macOS Now Playing registration: the supplied implementation never registered those controls. Menu Play/Pause, Next, and Quit are retained. No new Now Playing implementation was added.

Core/history composition is recorded in merge `34acca8`; menubar and packaging composition in `866510b` and `eb78506`. These resolutions are available for later upstream rebases with `git show --remerge-diff <merge>`. The combined player was constructed from the cleaned core boundary and the independent history operations, rather than copied wholesale from the original integration player.

## Test ownership ledger

All 73 distinct named tests declared in the original integration source are retained. The reconstructed integration declares 81 distinct named tests across platform/feature gates. Name/location accounting supplements the executed suites; it does not imply execution on Windows.

| Original test | Reconstructed location |
| --- | --- |
| `audio_bar_contains_percentage` | `src/tests/ui.rs` |
| `audio_bar_full_volume` | `src/tests/ui.rs` |
| `audio_bar_muted_volume` | `src/tests/ui.rs` |
| `base_works` | `src/tests/tracks.rs` |
| `binding_action_names_cover_every_binding_message` | `src/config.rs` |
| `bindings_are_case_insensitive_and_ctrl_c_is_reserved` | `src/ui/input.rs` |
| `built_in_defaults_are_identified_as_the_source` | `src/config.rs` |
| `bundled_icon_rasterizes_at_menu_bar_sizes` | `src/menubar.rs` |
| `closed_command_channel_returns_an_error` | `src/ui/mpris.rs` |
| `controls_has_items` | `src/tests/ui.rs` |
| `custom_display_with_exclamation` | `src/tests/tracks.rs` |
| `decoded_fails_with_invalid_audio` | `src/tests/tracks.rs` |
| `decodes_url` | `src/tests/tracks.rs` |
| `defaults_preserve_every_existing_binding` | `src/config.rs` |
| `download_and_decode_local_mp3_fixture` | `src/tests/tracks.rs` |
| `duplicate_bookmark_removes` | `src/tests/bookmark.rs` |
| `file_environment_and_cli_precedence` | `src/config.rs` |
| `file_overrides_replace_individual_binding_lists` | `src/config.rs` |
| `format_duration_hours_wrap` | `src/tests/ui.rs` |
| `format_duration_works` | `src/tests/ui.rs` |
| `format_duration_zero` | `src/tests/ui.rs` |
| `handles_all_numeric_name` | `src/tests/tracks.rs` |
| `handles_extension_only` | `src/tests/tracks.rs` |
| `history_has_no_previous_at_startup` | `src/history.rs` |
| `history_is_bounded_and_evicts_oldest_track` | `src/history.rs` |
| `history_keeps_forward_entries_after_going_back` | `src/history.rs` |
| `history_returns_to_skipped_track_before_downloading` | `src/history.rs` |
| `interface_mode_accepts_environment_override` | `src/config.rs` |
| `interface_mode_helpers_preserve_terminal_and_menu_choices` | `src/config.rs` |
| `invalid_and_duplicate_bindings_are_rejected` | `src/config.rs` |
| `invalid_cli_values_identify_the_cli_flag_as_the_source` | `src/config.rs` |
| `invalid_values_identify_their_source` | `src/config.rs` |
| `loaded_is_ignored_after_a_track_has_started` | `src/player.rs` |
| `loading` | `src/tests/ui.rs` |
| `loading_bookmark_is_ignored_and_quit_stops_dispatch` | `src/player.rs` |
| `loading_guards_and_playback_view_reflect_live_state` | `src/player.rs` |
| `malformed_unknown_and_reserved_bindings_fail` | `src/config.rs` |
| `matching_generation_advances_and_stale_messages_preserve_transition_state` | `src/player.rs` |
| `media_events_map_without_initializing_native_controls` | `src/ui/media_controls.rs` |
| `media_keys_and_unmapped_input_are_handled_explicitly` | `src/ui/input.rs` |
| `menu_mode_requires_the_macos_feature` | `src/config.rs` |
| `menu_modes_load_on_macos_with_the_menubar_feature` | `src/config.rs` |
| `multiple_bookmarks` | `src/tests/bookmark.rs` |
| `new_border_strings` | `src/tests/ui.rs` |
| `new_trims_lines` | `src/tests/tracks.rs` |
| `only_explicit_cli_values_override_configuration` | `src/config.rs` |
| `only_volume_updates_start_the_volume_timer` | `src/ui.rs` |
| `paused_muted_and_bookmarked_tracks_keep_their_rendering` | `src/tests/ui.rs` |
| `playback_commands_are_forwarded_to_the_command_channel` | `src/ui/mpris.rs` |
| `playback_commands_transition_valid_tracks_and_preserve_pause` | `src/player.rs` |
| `playback_volume_and_rate_publish_ordered_updates` | `src/player.rs` |
| `player_properties_and_metadata_follow_playback_state` | `src/ui/mpris.rs` |
| `previous_capability_and_dispatch_follow_history_state` | `src/ui/mpris.rs` |
| `previous_restart_threshold_is_three_seconds` | `src/history.rs` |
| `previous_restarts_a_rendered_track_after_three_seconds` | `src/player.rs` |
| `progress` | `src/tests/ui.rs` |
| `publishing_without_a_subscriber_is_an_error` | `src/player.rs` |
| `queued_generates_display_if_none` | `src/tests/tracks.rs` |
| `queued_uses_custom_display` | `src/tests/tracks.rs` |
| `random_path_no_display` | `src/tests/tracks.rs` |
| `random_path_parses_custom_display` | `src/tests/tracks.rs` |
| `rate_changes_are_forwarded_to_the_command_channel` | `src/ui/mpris.rs` |
| `simple` | `src/tests/ui.rs` |
| `single_track` | `src/tests/tracks.rs` |
| `spaced` | `src/tests/ui.rs` |
| `stale_generation_messages_are_ignored` | `src/player.rs` |
| `to_entry_roundtrip` | `src/tests/tracks.rs` |
| `toggle_and_check` | `src/tests/bookmark.rs` |
| `track` | `src/tests/ui.rs` |
| `tray_icon_constructs_from_the_retina_raster` | `src/menubar.rs` |
| `volume` | `src/tests/ui.rs` |
| `width_counts_graphemes` | `src/tests/tracks.rs` |
| `zero_width_window` | `src/tests/ui.rs` |

Existing configuration/input regression tests move with Figment. Baseline deterministic download, native adapter mapping/property tests, and shared fixtures move with Nextest. Core owns neutral dispatch/publication, rate forwarding, and renderer tests. History owns navigation, generation, waiter, and Previous capability tests. Menubar owns interface modes, snapshots, and icon tests. Packaging retains its executable verification script. Feature-specific assertions are extended during integration so both core and history expectations remain enforced.

## Verification

- Standard Cargo tests passed on all eight PR branches and integration. Default integration: **72 passed**.
- Nextest passed with default and disabled default features on the testing baseline, every feature branch, and integration. Curated `cargo fc check --locked --all-targets` and default mutation inventory generation passed on the same branches.
- Native macOS integration with `menubar,macos-app,scrape,extra-audio-formats`: **74 passed**, with both default and disabled default features. Final standalone menubar and packaging tips also passed both native modes.
- Native Linux/aarch64 container tests with `mpris,scrape,extra-audio-formats` passed on the testing baseline, core, history, menubar, packaging, and integration, with both default and disabled default features. Integration: **77 passed** in each mode. Source snapshots used unique paths to prevent stale Cargo artifacts.
- Native Linux MPRIS mutation inventories passed with pinned cargo-mutants 27.1.0 on Nextest, core, history, and integration. Mutation inventories were generated; full mutation passes were not run.
- `actionlint` passed for the combined workflow set.
- The optional application launcher passed `cargo check --locked --features macos-app --bin lowfi-app-launcher` on native Linux.
- The standalone packaging branch and integration each built and verified `Lowfi.app`: plist values, executable permissions, icon conversion, help/version execution, relocation with spaces/Unicode/apostrophes, and missing-executable failure. Generated bundles remain in a task-specific target directory; nothing was installed.

### Known validation limits

- AGENTS retains upstream rustfmt differences in `src/ui/init.rs` and `src/ui/media_controls.rs`; Figment retains the upstream differences in `src/ui/media_controls.rs`. Their checks were run and the inherited failures recorded. Unrelated Rust formatting was kept out of those PRs. CI and Nextest include the relevant formatting corrections, so all later branches and integration pass formatting.
- Native Windows execution remains pending CI. Native Windows feature and adapter checks are configured, but this macOS host has no Windows runtime.
- Live native event loops, audio-device playback, D-Bus property publication, and SMTC desktop integration were not exercised. Their existing documented mutation exclusions remain.

## Upstream PR sequence

Prepare AGENTS, CI, and Figment independently against upstream main. Land Figment before Nextest. Once Nextest lands, core, history, and packaging can be proposed independently; menubar waits for core. Rebase onto the actual merged upstream commits before opening dependent PRs so prerequisites do not inflate their diffs. Reuse the recorded integration resolutions when core and history both land. Do not submit the integration branch or this local audit as an upstream PR.

Raw source snapshots, preservation hashes, check records, and logs are retained locally at `/private/tmp/lowfi-stack-audit`. The attached managed worktree is `/Users/asmith/.codex/worktrees/upstream-pr-stack/lowfi`.

## Maintainer documentation pass for PRs #136–#143

This section records the later documentation pass. Earlier statements about unpublished branches describe the original reconstruction phase; the eight upstream PRs now exist. Their titles, bases, and draft/ready states are preserved.

### Recovery and scope

The pass retained upstream base `d86a51e93375efcfa10b5b98878113989e1b673f`. Source refs, fork heads, actual parent merge-bases, PR metadata, original worktree status, and hashes of dirty files were captured at `20261030T031301Z`. Recoverable refs are under `refs/backup/doc-pass/20261030T031301Z/`. Raw snapshots, checks, and logs are retained locally at `/private/tmp/lowfi-doc-pass`.

Parents were updated before children. The approved Figment configuration-reference/help commit `8bfcc70` now reaches Nextest and every descendant. Integration was rebuilt from the pinned base in the original merge order, using the recorded core/history resolutions. Every refreshed feature tip is an ancestor of the rebuilt integration.

| PR | Final feature tip | Documentation outcome |
| --- | --- | --- |
| #136 AGENTS | `fb3eda3a313c9061c88750ddb9347530258666e8` | Deliberate no-op: guidance agrees with contributor policy. |
| #137 CI | `add0001b507a3d3948e3c612871264ce1d5d37fb` | Explain native feature selection and attribute the extra Windows row to Lowfi. |
| #138 Figment | `42579bc5c824a9cc0d784cb680ed3ab6c35555b7` | Explain loading/validation contracts, explicit CLI overrides, binding lookup/list replacement, and legacy switches; link to the options reference. |
| #139 Testing | `6e23a61d588bc2410023dbf216982bb6ed638479` | Explain fixture ownership, bounded single-request serving, cancellation/join behavior, native/mutation boundaries, and repair shared rustdoc links. |
| #140 Core | `4018f1f34526a4428080b9d5c13aee40bed86cec` | Explain live views, copied metadata, update ordering, receiver lifetime, command outcomes, and adapter forwarding. |
| #141 History | `67e234e21a00f612f97b468e1e5c104f74284a92` | Explain total capacity, cursor/forward retention, restart/loading semantics, generation changes, coalescing, and stale events. |
| #142 Menubar | `15d74f6ce13fbb3e62065a9d013055700792ab19` | Explain main-thread/worker ownership, cleanup, loop termination, coalesced snapshots, and sampled position. |
| #143 Packaging | `ac49c946d2253abc67fb517523ed9bec11490b90` | Explain the Finder-to-player launch chain, bundle-relative paths, argument handling, and verification boundaries. |

The source changes are comments and documentation. Exact non-comment comparisons against the captured tips found no executable, dependency, asset, feature-gate, terminal-output, or workflow-behavior changes. The integration comparison explicitly allows the already-approved Figment reference/README/help changes. Original non-stack branches and the status/content hashes of original worktrees were checked and preserved. Separate read-only reviews checked each area and the combined integration against its implementation and existing tests. No library target or doctest infrastructure was added.

### Verification

The combined code/documentation snapshot was `3b0208c4f0b5bb61303396db4e6181bd19d16aaa`; this ledger addition changes Markdown only.

- `cargo test --locked` passed on every PR branch: AGENTS 33, CI 33, Figment 46, Nextest 46, core 55, history 60, menubar 59, packaging 46. Integration passed 72 default tests.
- `cargo fmt --check` passed on CI and Nextest onward, including integration. AGENTS retains the known upstream differences in `src/ui/init.rs` and `src/ui/media_controls.rs`; Figment retains those in `src/ui/media_controls.rs`. No unrelated formatting was added.
- Private-item `cargo doc --locked --no-deps --document-private-items` builds passed. Early branches retain their recorded inherited broken-link warnings; Nextest and every descendant, including integration, passed with `-D rustdoc::broken_intra_doc_links`.
- Default and no-default Nextest suites, curated `cargo fc check --locked --all-targets` checks, and mutation inventory generation passed on Nextest and every descendant, including integration. Full mutation passes were not run for this comment-only pass.
- Native macOS feature documentation passed. Feature-enabled menubar, packaging, and integration suites passed with and without default features; combined integration passed 74 tests in each mode.
- Linux/aarch64 container documentation passed with native MPRIS features on Nextest and every descendant. Combined integration passed 77 tests in each default/no-default mode with `mpris,scrape,extra-audio-formats`.
- Packaging shell syntax checks and integration `actionlint` passed.

Native Windows execution remains unverified locally. This pass did not rerun live audio, AppKit, D-Bus/SMTC desktop integration, or release-bundle/GUI verification; prior reconstruction and manual PR evidence remains separate. Binary targets still disable doctests, so Rust examples are not presented as executed doctests.

### Publication

Publication uses explicit branch refs and the captured remote SHA in each `--force-with-lease`, after complete local verification. The integration branch remains a fork-only review/testing branch; this audit is not added to any upstream PR. PR bodies are updated only to describe the documentation or current validation, preserving existing rationale and manual test evidence.

