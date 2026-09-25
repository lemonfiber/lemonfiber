//! The world most requests here run against: nothing runs, no stack is on disk, and the
//! randomness is a test's own.

use std::sync::Arc;

use lemonfiber_core::app::Ctx;
use lemonfiber_fixtures::ports::{Chance, Idle};
use lemonfiber_fixtures::pulled::Pulled;

/// A context that needs neither a stack on disk nor a daemon to answer.
///
/// What the engine has pulled is faked too, so a start's port pre-flight never reaches
/// a real daemon: what else runs on the machine a test happens to run on is not a fact
/// any test here is about.
pub(crate) fn ctx() -> Ctx {
    lemonfiber_testing::a_live_context()
        .runner(Arc::new(Idle))
        .images(Pulled::holding(Vec::new()))
        .over(lemonfiber_testing::nowhere())
        .build()
        .with_random(Arc::new(Chance::cycling()))
}
