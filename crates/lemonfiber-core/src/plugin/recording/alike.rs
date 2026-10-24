//! Whether a recording holds what was taken again, where a service makes some values
//! afresh on every run: an identifier or a moment is held to its shape alone.

use serde_json::Value;

/// A shape a value a service makes afresh on every run takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// A UUID, in its hyphenated form.
    Uuid,
    /// 32 hexadecimal digits.
    Hex,
    /// An RFC 3339 date and time.
    Moment,
}

/// Whether `held` holds what `taken` does, each string in a shape a service makes afresh
/// on every run differing only where its shape does.
pub(super) fn alike(held: &Value, taken: &Value) -> bool {
    match (held, taken) {
        (Value::String(held), Value::String(taken)) => {
            held == taken || shaped(held).is_some_and(|shape| shaped(taken) == Some(shape))
        }
        (Value::Array(held), Value::Array(taken)) => {
            held.len() == taken.len()
                && held.iter().zip(taken).all(|(one, other)| alike(one, other))
        }
        (Value::Object(held), Value::Object(taken)) => {
            held.len() == taken.len()
                && held
                    .iter()
                    .all(|(key, one)| taken.get(key).is_some_and(|other| alike(one, other)))
        }
        _ => held == taken,
    }
}

/// The shape `text` takes, where it is one a service makes afresh on every run.
fn shaped(text: &str) -> Option<Shape> {
    if uuid(text) {
        Some(Shape::Uuid)
    } else if text.len() == 32 && text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(Shape::Hex)
    } else if text.contains('T') && text.parse::<jiff::Timestamp>().is_ok() {
        Some(Shape::Moment)
    } else {
        None
    }
}

/// Whether `text` is a UUID in its hyphenated form.
fn uuid(text: &str) -> bool {
    let groups: Vec<&str> = text.split('-').collect();
    groups.len() == 5
        && groups.iter().zip([8, 4, 4, 4, 12]).all(|(group, len)| {
            group.len() == len && group.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

#[cfg(test)]
mod tests;
