use super::reads;

#[test]
fn a_ratio_reads_as_the_client_would_write_it() {
    assert_eq!(reads(175).as_deref(), Some("1.75"));
    assert_eq!(reads(0).as_deref(), Some("0.00"));
    assert_eq!(reads(7).as_deref(), Some("0.07"));
    assert_eq!(reads(1_000).as_deref(), Some("10.00"));
    assert_eq!(
        reads(u32::MAX),
        None,
        "a torrent that downloaded nothing has no ratio to divide"
    );
}
