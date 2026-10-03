//! How every one of these files is written and read.

use serde::Serialize;

use crate::Unreadable;

/// `value` as one of these files is written: indented JSON ending in a newline.
pub(crate) fn written(value: &impl Serialize) -> String {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
    text.push('\n');
    text
}

/// `text` read as the shape the file named `file` should hold.
pub(crate) fn read<T: serde::de::DeserializeOwned>(
    file: &'static str,
    text: &str,
) -> Result<T, Unreadable> {
    serde_json::from_str(text).map_err(|error| unreadable(file, &error.to_string()))
}

/// Whether `format` is `expected`, the one this build reads for `file`.
pub(crate) fn formatted(file: &'static str, format: u32, expected: u32) -> Result<(), Unreadable> {
    if format == expected {
        Ok(())
    } else {
        Err(unreadable(
            file,
            &format!("it is written in format {format}, and this build reads format {expected}"),
        ))
    }
}

/// `file` could not be read, because `why`.
pub(crate) fn unreadable(file: &'static str, why: &str) -> Unreadable {
    Unreadable {
        file,
        why: why.to_owned(),
    }
}
