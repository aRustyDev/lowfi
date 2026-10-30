# Playback and terminal boundary

The application owns one playback session and routes commands to the player.
The player owns decoding, playback, bookmarks, and persistence, and publishes
frontend-neutral updates. Read-only playback views expose volume, position,
pause state, and rate. Terminal rendering layers its display state over these
updates without changing the terminal layout or controls.

Linux MPRIS and Windows media controls retain their optional feature gates and
native implementations. MPRIS can run with the terminal disabled. Native
adapters share the existing command channel and observe the same playback state.

The optional macOS `menubar` feature adds a native menu frontend. AppKit and its
event loop stay on the main thread. A worker owns the playback session and
terminal lifecycle; it forwards immutable snapshots to the menu and accepts
commands through the existing application channel. Worker completion closes the
native loop. Menu-only mode starts no terminal input or raw-mode setup.
