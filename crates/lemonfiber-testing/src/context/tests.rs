use std::path::PathBuf;

use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::scratch::Scratch;

use super::{a_context, Holds};

#[test]
fn a_held_directory_goes_when_the_last_context_holding_it_drops() {
    let scratch = Scratch::new("held");
    let dir = scratch.to_path_buf();
    let ctx = a_context().holding(scratch).build();
    let clone = ctx.clone();

    drop(ctx);
    let after_one = dir.exists();
    drop(clone);

    assert!(after_one);
    assert!(!dir.exists());
}

#[test]
fn a_directory_held_after_the_build_goes_with_the_last_clone_and_the_earlier_ones_too() {
    let before = Scratch::new("held-before");
    let after = Scratch::new("held-after");
    let (first, second) = (before.to_path_buf(), after.to_path_buf());
    let ctx = a_context().holding(before).build().holding(after);
    let clone = ctx.clone();

    drop(ctx);
    let after_one = first.exists() && second.exists();
    drop(clone);

    assert!(after_one);
    assert!(!first.exists());
    assert!(!second.exists());
}

#[test]
fn a_held_directory_goes_when_the_test_holding_it_panics() {
    let scratch = Scratch::new("held-panicking");
    let dir = scratch.to_path_buf();
    let ctx = a_context().holding(scratch).build();

    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _held = ctx;
        std::panic::resume_unwind(Box::new("the test failed"));
    }));

    assert!(unwound.is_err());
    assert!(!dir.exists());
}

#[test]
fn the_stack_a_context_is_written_apart_goes_with_it() {
    let ctx = a_context().build();
    let stack = match &ctx.stack {
        Source::External(at) => at.to_path_buf(),
        Source::Embedded(_) => PathBuf::new(),
    };

    let written = stack.is_dir();
    drop(ctx);

    assert!(written);
    assert!(!stack.exists());
}

#[test]
fn a_context_over_a_stack_directory_reads_it_and_holds_it_until_it_drops() {
    let scratch = Scratch::new("stack-in");
    let dir = scratch.to_path_buf();
    let ctx = a_context().stack_in(scratch).build();
    let read = match &ctx.stack {
        Source::External(at) => at.to_path_buf(),
        Source::Embedded(_) => PathBuf::new(),
    };

    drop(ctx);

    assert_eq!(read, dir);
    assert!(!dir.exists());
}

#[test]
fn a_context_holding_what_another_holds_keeps_it_until_both_drop() {
    let scratch = Scratch::new("held-what");
    let dir = scratch.to_path_buf();
    let first = a_context().holding(scratch).build();
    let second = a_context().build().holding_what(&first);

    drop(first);
    let kept = dir.exists();
    drop(second);

    assert!(kept);
    assert!(!dir.exists());
}
