//! What the operator chose, and where it is kept.
//!
//! Reading and writing the environment file preserves comments and ordering,
//! because it is a file an operator edits by hand and a rewrite that reorders it
//! destroys their annotations.
//!
//! Configuration written by a newer build is refused rather than modified.
//! Silently downgrading a config file is how a downgrade-to-test becomes an
//! unrecoverable state.
//!
//! Arrives with the setup wizard. See `.docs/architecture/module-layout.md`.
