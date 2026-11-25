//! A seeding ratio as a person reads it.

/// A seeding ratio as a person reads it, from the hundredths the client reports.
///
/// Nothing where there is no ratio to read: a torrent added from files already on
/// disk downloaded nothing, so what it has given back is not divisible by what it
/// took, and the largest figure this can carry stands for that. Printing that
/// figure would show somebody forty-two million, which is a number rather than an
/// answer — so the caller says the thing in words instead.
#[must_use]
pub fn reads(hundredths: u32) -> Option<String> {
    (hundredths != u32::MAX).then(|| format!("{}.{:02}", hundredths / 100, hundredths % 100))
}

#[cfg(test)]
mod tests;
