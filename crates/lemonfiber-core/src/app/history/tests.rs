use super::{did, stamped};

/// A region reads as what it was: something written into a file that was there.
#[test]
fn a_region_reads_as_whose_it_is_and_which_file_it_went_into() {
    assert_eq!(
        did(&crate::journal::Kind::Region {
            path: "/stack/config/caddy/Caddyfile".to_owned(),
            key: "config/caddy/Caddyfile".to_owned(),
            owner: "plugin komga".to_owned(),
            written: 0,
        }),
        "wrote plugin komga's region into /stack/config/caddy/Caddyfile"
    );
}

/// Every stamp this build writes goes out as it was written.
#[test]
fn a_stamp_of_seconds_goes_out_as_written() {
    assert_eq!(stamped("1709287200"), "1709287200");
    assert_eq!(stamped("0"), "0");
}

/// An earlier build wrote an empty stamp where its clock would not answer, and a
/// journal outlives the build that wrote it. The report promises digits, so what is
/// not digits is read as the epoch — this build's own spelling of that clock —
/// rather than passed on as a promise it does not keep.
#[test]
fn a_stamp_that_is_not_seconds_is_read_as_the_epoch() {
    for unreadable in ["", "t", "2024-03-01T10:00:00Z", "-1", "12a"] {
        assert_eq!(stamped(unreadable), "0", "{unreadable:?}");
    }
}
