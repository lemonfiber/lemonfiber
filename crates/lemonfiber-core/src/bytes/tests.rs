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
