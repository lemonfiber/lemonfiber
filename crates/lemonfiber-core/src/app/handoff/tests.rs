use super::code;

/// An app that takes a link is handed its link, with the address where the link asks
/// for it; one that takes none is handed the address alone.
#[test]
fn a_link_carries_the_address_and_no_link_is_the_address() {
    let address = "http://192.168.1.20:8096";

    assert_eq!(
        code(Some("an-app://open?server={address}"), address),
        "an-app://open?server=http://192.168.1.20:8096"
    );
    assert_eq!(code(None, address), address);
}
