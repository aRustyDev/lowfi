# Lowfi contributor guidance

- Use Rust 1.83+ and the Rust 2021 edition.
- Verify changes with `cargo fmt --check` and `cargo test`.
- Preserve Lowfi's terminal UI; do not redesign it or add visual complexity.
- Keep OS-specific integrations optional and feature-gated.
- Preserve the existing Linux MPRIS implementation; unrelated platform work must not replace it.
- Keep pull requests small, focused, and independently reviewable.
- Do not modify unrelated files or existing user changes.
- Follow [CONTRIBUTING.md](CONTRIBUTING.md) as the authoritative contribution policy.
