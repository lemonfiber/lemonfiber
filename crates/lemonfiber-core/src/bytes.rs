//! A byte count as a person reads it.
//!
//! Here rather than in the check that first needed it, because the second one that
//! needs it must not spell it differently: an operator comparing a storage figure with
//! a provider one has to be able to trust that both mean the same thing by a gibibyte.

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
    use super::humanize;

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
