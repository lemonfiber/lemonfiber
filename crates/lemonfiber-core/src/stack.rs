//! Building and running a slice of the stack.
//!
//! Two responsibilities that must not merge. Building the Compose argument
//! vector is a pure function over the manifest, the configuration and the
//! environment; running it is a thin layer above [`crate::ports::Runner`].
//!
//! Keeping construction pure is what lets every form on every platform be
//! covered by golden files with no daemon present, and it is why a rehearsal
//! and a real run cannot disagree — they are the same function.
//!
//! Form closure is resolved before intersecting with the protocols the operator
//! actually configured, in that order: a download form resolves to both usenet
//! and torrent, then narrows to what exists, so a tunnel is never started with
//! credentials that were never supplied.
//!
//! Arrives with the compose driver. See `.docs/architecture/module-layout.md`.
