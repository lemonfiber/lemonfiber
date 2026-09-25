//! The context builder as the core's own tests reach it.
//!
//! The same file is built into the testing crate, where its own tests cover it; this
//! copy is compiled into the core, so the parts no core test otherwise names are held
//! here.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::pulled::Pulled;

use crate::ports::docker::Images;
use crate::ports::FileSystem;

use super::context::{a_context, a_live_context};

#[test]
fn a_filesystem_and_an_image_list_named_are_the_ones_the_run_reaches() {
    let files: Arc<dyn FileSystem> = Files::empty();
    let pulled: Arc<dyn Images> = Pulled::holding(Vec::new());

    let ctx = a_context()
        .filesystem(Arc::clone(&files))
        .images(Arc::clone(&pulled))
        .build();

    assert!(Arc::ptr_eq(&ctx.seams.filesystem, &files));
    assert!(Arc::ptr_eq(&ctx.seams.images, &pulled));
}

#[test]
fn a_live_context_reads_the_clock_on_the_wall() {
    let wall = SystemTime::now();
    let read = a_live_context().build().seams.clock.now();

    let apart = read
        .duration_since(wall)
        .unwrap_or_else(|behind| behind.duration());
    assert!(
        apart < Duration::from_secs(60),
        "{apart:?} from the wall clock"
    );
}
