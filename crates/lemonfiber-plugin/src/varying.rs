//! The values a service makes afresh on every run, each named by its place in the answer
//! and held to a shape there alone.
//!
//! A request's `varies` maps a JSON Pointer into its answer to an RE2 pattern the whole
//! value there matches. A recording taken again holds that one place to that shape, and
//! every other value to exactly what was recorded.

use serde_json::Value;

use crate::pointing::{self, Step};

/// The most bytes a shape may hold.
pub const SHAPE_MOST: usize = 256;

/// The most memory a compiled shape may take, in bytes.
const COMPILED_MOST: usize = 1 << 20;

/// `shape`, compiled to match a whole value.
///
/// # Errors
///
/// Why it is no shape: longer than [`SHAPE_MOST`], or a pattern that does not compile
/// within its bound.
pub fn shape(shape: &str) -> Result<regex::Regex, String> {
    if shape.len() > SHAPE_MOST {
        return Err(format!(
            "{shape} is longer than the {SHAPE_MOST} bytes a shape may hold"
        ));
    }
    regex::RegexBuilder::new(&format!("^(?:{shape})$"))
        .size_limit(COMPILED_MOST)
        .dfa_size_limit(COMPILED_MOST)
        .build()
        .map_err(|_| format!("{shape} is not an RE2 pattern this build compiles"))
}

/// Whether `place` is a JSON Pointer naming a member at each step.
///
/// # Errors
///
/// Why it names no place a value varies at: not a pointer, one that is malformed, or one
/// picking a list's entry by a field, which names an entry by what it holds rather than
/// where it is.
pub fn place(place: &str) -> Result<(), String> {
    if !place.starts_with('/') {
        return Err(format!(
            "{place} is not a JSON Pointer; a place a value varies at is written /member/member"
        ));
    }
    let steps = pointing::steps(place).map_err(|malformed| malformed.to_string())?;
    if steps
        .iter()
        .any(|step| matches!(step, Step::Selected { .. }))
    {
        return Err(format!(
            "{place} picks a list's entry by a field it holds; a place a value varies at names \
             each step by where it is"
        ));
    }
    Ok(())
}

/// Whether `value` is a string the whole of which `shape` matches.
#[must_use]
pub fn held(shape: &str, value: Option<&Value>) -> bool {
    let compiled = self::shape(shape);
    matches!((compiled, value), (Ok(compiled), Some(Value::String(text))) if compiled.is_match(text))
}

#[cfg(test)]
mod tests;
