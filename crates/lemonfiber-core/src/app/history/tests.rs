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

/// A file written over reads as what it was: written over, naming the file.
#[test]
fn a_file_written_over_reads_as_which_file_it_was() {
    assert_eq!(
        did(&crate::journal::Kind::Rewritten {
            path: "/stack/compose/plugins/komga.yml".to_owned(),
            previous: String::new(),
            written: 0,
        }),
        "wrote over /stack/compose/plugins/komga.yml"
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

/// A key is named with its scope, minted or revoked.
#[test]
fn a_key_reads_as_its_name_and_scope() {
    assert_eq!(
        did(&crate::journal::Kind::KeyMinted {
            name: "ha".to_owned(),
            scope: "act".to_owned(),
        }),
        "minted the key ha, with the scope act"
    );
    assert_eq!(
        did(&crate::journal::Kind::KeyRevoked {
            name: "ha".to_owned(),
            scope: "act".to_owned(),
        }),
        "revoked the key ha, with the scope act"
    );
}
