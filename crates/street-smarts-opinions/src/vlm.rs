//! The VLM opinion family -- deferred to v0.2+ per `SPEC.md` §5.2.
//!
//! This module exists now, empty, purely to establish the Cargo feature
//! boundary (`vlm`, in this crate's `Cargo.toml`) *before* any opinion
//! that needs `Capability::Network` is implemented, rather than after --
//! see `HARDENING_SPEC.md` §3. `street-smarts-web`'s default build never
//! enables the `vlm` feature, so anything added here is structurally
//! absent from the activist-facing WASM bundle regardless of what it
//! ends up containing.
//!
//! When this family is built: every opinion here must return
//! `&[Capability::Network]` (or `&[Capability::Network, Capability::ApiKey]`)
//! from `Opinion::capabilities()`. That's the second half of the
//! enforcement -- the feature wall keeps it out of the default build; the
//! capability declaration keeps it honestly labeled everywhere else.
