use super::bundled;

#[test]
fn the_bundled_roots_build_a_configuration() {
    // The trust anchors a static binary carries rather than reading from a
    // system store, so a release with no system TLS still verifies a server.
    assert!(bundled().is_ok());
}
