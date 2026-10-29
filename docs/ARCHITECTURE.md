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

Future work: Spotlight integration, app bundles, signing, notarization,
launchd lifecycle management, and IPC for independently attached clients.
