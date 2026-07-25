//! Checks that prove things rather than assuming them.
//!
//! Every check is independent, bounded by its own timeout, and returns a finding
//! that carries a remedy. Checks are values in a collection rather than
//! branches in a function, so one that hangs or fails cannot take the others
//! with it.
//!
//! "Could not check" is a distinct variant of the finding type rather than a
//! severity value, so it cannot accidentally render as "passed" — which is the
//! failure that would make the whole feature dishonest.
//!
//! An error inside a check surfaces as a check error, never as a finding about
//! the stack.
//!
//! Arrives with the diagnostics. See `.docs/architecture/module-layout.md`.
