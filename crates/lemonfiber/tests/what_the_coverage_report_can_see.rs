//! What the coverage report can and cannot watch being decided.
//!
//! Its own file rather than more of `architecture.rs`, and not only because that one
//! is at its length cap: the rules there are about where a *name* may appear, and this
//! is about where a *decision* may sit. A reader looking for one is not looking for
//! the other.

mod source_tree;

use source_tree::{production, sources};

/// Nothing decides anything inside an `#[async_trait]` method body.
///
/// `#[async_trait]` rewrites a method body into a generated future, and the coverage
/// report attributes nothing inside it to the lines it came from. The signature carries
/// a count and every line beneath it carries none, so a branch in there can go untaken
/// for ever and the gate that says this workspace is fully covered will not say a word.
///
/// Shown before it was relied on: the same never-taken branch planted inside such a
/// method got no coverage region at all, while the identical branch in a plain `fn` and
/// in a plain `async fn` was mapped, counted zero and reported.
///
/// Only branching. Straight-line code loses nothing — if the method ran, those lines
/// ran, and the signature's count says so. And a closure inside the body is fine: it
/// compiles to a function item of its own, which the report does see. What is refused is
/// a bare `if`, `match`, `for`, `while`, `loop` or `return`, where a line can be skipped
/// while the method still runs and nothing anywhere reports it.
///
/// The remedy is never an exemption. Move the body to a plain `async fn` beside the
/// impl and delegate to it: the asynchrony is unchanged, and the decision lands
/// somewhere the gate can watch it being made.
#[test]
fn nothing_decides_inside_a_body_the_coverage_report_cannot_see() {
    let mut hidden: Vec<String> = Vec::new();
    let mut seen = 0_usize;

    for (path, text) in sources() {
        if path.to_string_lossy().contains("tests") {
            continue;
        }
        for (line, body) in async_trait_bodies(production(&text)) {
            seen += 1;
            if branches_outside_a_closure(&body) {
                hidden.push(format!("{}:{line}", path.display()));
            }
        }
    }

    assert!(
        seen > 100,
        "the scan found {seen} bodies, which means it is looking in the wrong place"
    );
    assert!(
        hidden.is_empty(),
        "a decision here is one the coverage gate cannot watch being made; move the \
         body to a plain async fn and delegate: {}",
        hidden.join(", ")
    );
}

/// Every `async fn` body inside an `#[async_trait]` block, with the line it opens on.
fn async_trait_bodies(shipped: &str) -> Vec<(usize, Vec<String>)> {
    let lines: Vec<&str> = shipped.lines().collect();
    let mut found = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        if lines.get(at).map(|line| line.trim()) != Some("#[async_trait]") {
            at += 1;
            continue;
        }
        let end = block_end(&lines, at + 1);
        let mut here = at + 1;
        while here < end {
            let opens_a_method = lines
                .get(here)
                .is_some_and(|line| line.trim_start().starts_with("async fn"));
            if !opens_a_method {
                here += 1;
                continue;
            }
            // A signature may span lines, so the brace that opens the body is the
            // first one at or after it — not the one on the line it began on. The
            // reading that missed this missed eighty-four methods.
            let opens = lines
                .get(here..end)
                .and_then(|rest| rest.iter().position(|line| line.contains('{')))
                .map_or(here, |found| here + found);
            let closes = block_end(&lines, opens).min(end);
            if let Some(body) = lines.get(opens + 1..closes) {
                if !body.is_empty() {
                    found.push((
                        here + 1,
                        body.iter().map(|line| (*line).to_owned()).collect(),
                    ));
                }
            }
            here = closes.max(here) + 1;
        }
        at = end + 1;
    }
    found
}

/// Where the block opening at or after `from` closes.
fn block_end(lines: &[&str], from: usize) -> usize {
    let mut depth = 0_i32;
    let mut opened = false;
    for (at, line) in lines.iter().enumerate().skip(from) {
        depth += i32::try_from(line.matches('{').count()).unwrap_or(0);
        if line.contains('{') {
            opened = true;
        }
        depth -= i32::try_from(line.matches('}').count()).unwrap_or(0);
        if opened && depth <= 0 {
            return at;
        }
    }
    lines.len().saturating_sub(1)
}

/// Whether a body decides anything outside a closure.
fn branches_outside_a_closure(body: &[String]) -> bool {
    let mut depth = 0_i32;
    let mut closure: Option<i32> = None;
    for line in body {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        if closure.is_none() && decides(trimmed) {
            return true;
        }
        if closure.is_none() && line.contains('|') && line.contains('{') {
            closure = Some(depth);
        }
        depth += i32::try_from(line.matches('{').count()).unwrap_or(0);
        depth -= i32::try_from(line.matches('}').count()).unwrap_or(0);
        if closure.is_some_and(|opened| depth <= opened) {
            closure = None;
        }
    }
    false
}

/// Whether a line opens a decision, as code rather than as words inside a string.
///
/// The quoted parts go first. A refusal whose wording happens to contain "if" is a
/// sentence an operator reads, not a branch, and a gate that cannot tell them apart is
/// one people learn to work around.
fn decides(trimmed: &str) -> bool {
    let code = outside_quotes(trimmed);
    let code = code.trim_start();
    ["if ", "match ", "for ", "while ", "loop", "return"]
        .iter()
        .any(|word| code.starts_with(word))
        || code.contains(" if ")
        || code.contains("= match ")
        || code.contains("=> match ")
}

/// A line with everything between double quotes taken out.
fn outside_quotes(line: &str) -> String {
    let mut kept = String::with_capacity(line.len());
    let mut quoted = false;
    let mut escaped = false;
    for ch in line.chars() {
        match ch {
            _ if escaped => escaped = false,
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            _ if !quoted => kept.push(ch),
            _ => {}
        }
    }
    kept
}
