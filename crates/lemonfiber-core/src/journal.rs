//! What lemonfiber changed, so it can be undone.
//!
//! Every write a subsystem makes to a service or to configuration is recorded
//! here with enough context to reverse exactly that change and nothing else.
//! Rolling back one change is the point; an all-or-nothing restore is what
//! backups are for.
//!
//! Arrives with seeding, which is the first subsystem that writes anything.
//! See `.docs/architecture/module-layout.md`.
