//! What is installed, read off the record alone or with each source asked.
//!
//! Apart from the verbs because both are readings: one asks nothing of any source and
//! can be read on a timer, and the other is what somebody listing what is installed
//! is answered with.

use crate::error::Problem;
use crate::plugin::Installs;

use super::super::Ctx;
use super::{fetching, read, standing};

/// What is installed and what each install decided, read off the record alone.
///
/// Nothing is asked of any source, so this is the reading a caller asking often can
/// afford: whether an installed plugin's source still answers is a question for
/// somebody listing what is installed, and asking it on a timer would reach every
/// repository on every tick.
///
/// # Errors
///
/// Where the record cannot be read. It is never read as nothing installed.
pub fn recorded(ctx: &Ctx) -> Result<Installs, Box<Problem>> {
    let held = read(ctx)?;
    Ok(Installs {
        nonconforming: super::conformance::held(ctx)?,
        proof: None,
        rehearsed: false,
        substituted: standing::substituted(
            held.installed(),
            &super::super::targets::chosen_fillers(ctx),
        ),
        sources: Vec::new(),
        installed: held.installed().to_vec(),
        install: None,
        removal: None,
        update: None,
        agreement: None,
    })
}

/// What is installed, as [`recorded`] reads it, with whether each source can still be
/// fetched, asked now.
///
/// # Errors
///
/// Where the record cannot be read.
pub async fn installed(ctx: &Ctx) -> Result<Installs, Box<Problem>> {
    let mut listed = recorded(ctx)?;
    listed.sources = fetching::standings(ctx, &listed.installed).await;
    Ok(listed)
}
