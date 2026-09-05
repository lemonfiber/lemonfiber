//! A byte count as a person reads it.
//!
//! Here rather than in the check that first needed it, because the second one that
//! needs it must not spell it differently: an operator comparing a storage figure with
//! a provider one has to be able to trust that both mean the same thing by a gibibyte.

/// A byte count read from the way a person writes one.
///
/// Binary units, matching what [`humanize`] writes, so a figure this product
/// printed can be handed back to it. The unit may be the bare letter a download
/// client writes or the `iB` a person reads, and a figure with no unit at all is a
/// count of bytes.
///
/// A fraction is taken to whatever precision it was written with. Anything that
/// overflows, or carries a unit this does not know, is `None` rather than a guess:
/// a limit read wrongly is a household wondering why the evening is ruined, and a
/// limit refused is a sentence.
#[must_use]
pub fn read(text: &str) -> Option<u64> {
    let text = text.trim();
    let split = text
        .find(|character: char| character.is_ascii_alphabetic())
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let scale: u64 = match unit.trim().to_ascii_uppercase().trim_end_matches("IB") {
        "" => 1,
        "K" => 1 << 10,
        "M" => 1 << 20,
        "G" => 1 << 30,
        "T" => 1 << 40,
        "P" => 1 << 50,
        _ => return None,
    };
    let (whole, fraction) = number.trim().split_once('.').unwrap_or((number.trim(), ""));
    let bytes = whole.trim().parse::<u64>().ok()?.checked_mul(scale)?;
    if fraction.is_empty() {
        return Some(bytes);
    }
    let places = u32::try_from(fraction.len()).ok()?;
    let scaled = fraction.parse::<u64>().ok()?.checked_mul(scale)?;
    bytes.checked_add(scaled / 10_u64.checked_pow(places)?)
}

/// A rate as a person reads it: a byte count with the second it is spread over.
///
/// Beside [`humanize`] rather than spelled at each place that shows one, because a
/// rate an operator compares against the figure on their provider's bill has to be
/// written the same way every time it is shown.
#[must_use]
pub fn a_second(bytes: u64) -> String {
    format!("{}/s", humanize(bytes))
}

/// A byte count as a person reads it, to one decimal place.
///
/// Binary units, because that is what the tools an operator will cross-check
/// against report, and one decimal because a library measured to the byte is
/// noise around a figure whose point is "roughly how much room is left".
#[must_use]
pub fn humanize(bytes: u64) -> String {
    const UNITS: [(&str, u64); 4] = [
        ("TiB", 1 << 40),
        ("GiB", 1 << 30),
        ("MiB", 1 << 20),
        ("KiB", 1 << 10),
    ];
    for (label, size) in UNITS {
        if bytes >= size {
            // Rounded to the nearest tenth rather than truncated, so 2.0 GiB does
            // not read as 1.9. The remainder is well under a terabyte, so scaling
            // it by ten cannot overflow; a remainder that rounds up to a whole
            // unit carries into it.
            let mut whole = bytes / size;
            let mut tenths = ((bytes % size) * 10 + size / 2) / size;
            if tenths == 10 {
                whole += 1;
                tenths = 0;
            }
            return format!("{whole}.{tenths} {label}");
        }
    }
    format!("{bytes} B")
}

#[cfg(test)]
mod tests {
    use super::{a_second, humanize, read};

    #[test]
    fn a_figure_this_printed_can_be_handed_back_to_it() {
        // One round trip or two dialects. An operator who reads a limit off one
        // command and types it into the next is owed the first.
        assert_eq!(read("1.5 KiB"), Some(1536));
        assert_eq!(read("3.0 MiB"), Some(3 * 1024 * 1024));
        assert_eq!(read("512"), Some(512));
        assert_eq!(read("2T"), Some(1024_u64.pow(4) * 2));
        assert_eq!(read("  10 g  "), Some(10 * (1 << 30)));
    }

    #[test]
    fn a_unit_this_does_not_know_is_refused_rather_than_guessed_at() {
        assert_eq!(read("5 furlongs"), None);
        assert_eq!(read(""), None);
        assert_eq!(read("-1M"), None);
        assert_eq!(read("999999999999999999999 T"), None);
        assert_eq!(read("1.9999999999999999999 T"), None);
    }

    #[test]
    fn a_rate_is_a_byte_count_with_the_second_it_is_spread_over() {
        assert_eq!(a_second(3 * 1024 * 1024), "3.0 MiB/s");
    }

    #[test]
    fn a_byte_count_reads_in_the_unit_a_person_would_use() {
        assert_eq!(humanize(0), "0 B");
        assert_eq!(humanize(512), "512 B");
        assert_eq!(humanize(1536), "1.5 KiB");
        assert_eq!(humanize(3 * 1024 * 1024), "3.0 MiB");
        assert_eq!(
            humanize(10 * 1024 * 1024 * 1024 + 512 * 1024 * 1024),
            "10.5 GiB"
        );
        assert_eq!(humanize(1024_u64.pow(4) * 2), "2.0 TiB");
        // Rounded, not truncated: a hair under two gigabytes reads as 2.0, and
        // the tenth that rounds up carries into the whole rather than showing 1.10.
        assert_eq!(humanize(2 * 1024 * 1024 * 1024 - 1), "2.0 GiB");
        assert_eq!(humanize(1024 * 1024 * 1024 + 550 * 1024 * 1024), "1.5 GiB");
    }
}
