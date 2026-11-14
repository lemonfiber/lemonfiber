//! A screen's loop awaits nothing but what it selects over.

use crate::source_tree::{production, sources};

/// Where the screens' loops live.
const SCREENS: &str = "crates/lemonfiber/src/terminal/";

/// The text of every `tokio::select!` in `text`, between its braces.
fn selected(text: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("tokio::select!") {
        let after = &rest[at..];
        let Some(open) = after.find('{') else {
            break;
        };
        let mut depth = 0_usize;
        let mut end = after.len();
        for (offset, letter) in after[open..].char_indices() {
            match letter {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + offset + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        found.push(&after[open..end]);
        rest = &after[end..];
    }
    found
}

#[test]
fn no_screen_awaits_inside_the_loop_that_draws_it() {
    let mut awaiting = Vec::new();
    let mut loops = 0_usize;
    for (path, text) in sources() {
        let where_it_lives = path.to_string_lossy().replace('\\', "/");
        if !where_it_lives.contains(SCREENS) {
            continue;
        }
        for block in selected(production(&text)) {
            loops += 1;
            if block.contains(".await") {
                awaiting.push(where_it_lives.clone());
            }
        }
    }
    assert!(
        loops >= 2,
        "found {loops} screen loops, so the search is looking in the wrong place"
    );
    assert!(
        awaiting.is_empty(),
        "a screen loop awaits inside the select that draws it, so a frame waits on it: \
         {awaiting:?}"
    );
}

#[test]
fn an_await_inside_a_select_is_found_and_one_outside_is_not() {
    let inside = "loop { tokio::select! { x = a.recv() => { f().await; } } }";
    let outside = "let y = g().await; tokio::select! { x = a.recv() => { h(x); } }";

    assert!(selected(inside)
        .iter()
        .any(|block| block.contains(".await")));
    assert!(!selected(outside)
        .iter()
        .any(|block| block.contains(".await")));
    assert_eq!(selected(outside).len(), 1);
}
