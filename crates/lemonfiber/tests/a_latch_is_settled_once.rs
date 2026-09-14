//! A value settled once at startup is never settled by the code that reads it.
//!
//! Its own file because the failure it guards against has no symptom. Four values
//! here are settled at startup and read everywhere after — what a terminal can
//! draw, who the output is for, whether words are explained, and which have been —
//! and a read reaching for `get_or_init` settles the value itself. The `settle`
//! that follows is then **silently ignored**, in whatever order some future caller
//! happens to introduce. Nothing reports it; the feature simply stops working for
//! that run.
//!
//! One of them was written that way once. Only a function that says it settles may.

mod source_tree;

use source_tree::{production, sources};

/// Reading a latch never settles it.
///
/// Four values here are settled once at startup and read everywhere after: what a
/// terminal can draw, who the output is for, whether words are explained, and which
/// have already been. A read that reached for `get_or_init` would settle the value
/// itself — and the `settle` that came afterwards would then be **silently ignored**,
/// in whatever order some future caller happened to introduce. Nothing reports that;
/// the feature simply stops working for that run.
///
/// One of them was written that way and this is why it is pinned. Only a function
/// that says it settles may settle.
#[test]
fn reading_a_latch_never_settles_it() {
    let mut settling: Vec<String> = Vec::new();
    for (path, text) in sources() {
        let where_it_lives = path.to_string_lossy().replace('\\', "/");
        if !where_it_lives.contains("/src/") {
            continue;
        }
        let mut whose = String::new();
        for (number, line) in production(&text).lines().enumerate() {
            if let Some(name) = declaring(line) {
                whose = name;
            }
            if line.contains("get_or_init") && !whose.starts_with("settle") {
                settling.push(format!("{where_it_lives}:{}: {whose}", number + 1));
            }
        }
    }
    assert!(
        settling.is_empty(),
        "a read that settles the value it reads, so a later settle is ignored: \
         {settling:?}"
    );
}
/// The name of the function a line declares, where it declares one.
fn declaring(line: &str) -> Option<String> {
    let (_, rest) = line.split_once("fn ")?;
    let name: String = rest
        .chars()
        .take_while(|letter| letter.is_alphanumeric() || *letter == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}
