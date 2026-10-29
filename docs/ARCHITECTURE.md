# Lowfi frontend boundary

The `lowfi` binary owns one playback session. The terminal frontend and the
optional macOS menu-bar frontend send commands through the same `Message`
channel and observe the same playback state. Player, downloader, configuration,
and persistence remain shared.

The macOS `menubar` feature compiles Tao, tray-icon, and the MediaPlayer adapter
only for `target_os = "macos"`. Tao owns AppKit's main-thread event loop. The
async player and audio stream run on a named worker thread, forwarding menu and
system-media commands to the existing message loop. The terminal remains the
default frontend and has no GUI dependencies in a normal build.

Linux MPRIS and Windows media controls retain their current feature gates and
implementations. The macOS adapter exposes only Play/Pause and Next; Lowfi does
not offer playback history through that interface.

The optional `macos-app` feature builds a small bundle launcher rather than a
new frontend. `Lowfi.app` starts the existing terminal player in Terminal.app;
the bundled command script resolves the adjacent `lowfi` executable at runtime,
so the bundle continues to work after it is moved or renamed.

Future work: signing, notarization, launchd lifecycle management, IPC for
independently attached clients, Windows Start-menu packaging, and Linux
`.desktop` integration.
