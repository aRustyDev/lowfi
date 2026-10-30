# Environment Variables

[[Version française](./fr/ENVIRONMENT_VARS.md)]

For persistent playback/UI settings and key bindings, see
[Configuration](CONFIGURATION.md). That reference covers `config.toml`,
`LOWFI_<SECTION>__<FIELD>` variables, and CLI precedence.

The switches below are separate legacy options, enabled only when their value
is exactly `1` (for example, `LOWFI_FIXED_MPRIS_NAME=1`). They are not fields in
`config.toml`.

lowfi has some more specific options, usually as a result of minor feature requests, which are only documented here.
If you have some behavior you'd like to change, which is quite specific, then see if one of these options suits you.

* `LOWFI_FIXED_MPRIS_NAME` - Limits the number of lowfi instances to one, but ensures the player name is always `lowfi`.
* `LOWFI_DISABLE_UI` - Disables the UI. This requires MPRIS, so that you can still actually control lowfi.
