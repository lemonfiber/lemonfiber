#[test]
fn a_speaker_answers_its_contracts_on_its_own_port_for_its_own_upstream_or_the_one_it_fronts() {
    let mut adapter = crate::test_support::a_placed("series", &["library.curate"], Some(8080));
    let upstream = crate::test_support::a_placed("series-upstream", &[], Some(8989));
    assert_eq!(adapter.contract_port(), None, "it speaks nothing");
    adapter.speaks = vec!["library.curate@1".to_owned()];
    assert_eq!(adapter.contract_port(), Some(8080));
    adapter.contract = Some(8081);
    assert_eq!(adapter.contract_port(), Some(8081));
    let curator = crate::test_support::an_installed("series", vec![adapter.clone(), upstream]);
    assert_eq!(
        curator
            .upstream_of(&adapter)
            .map(|one| one.service.as_str()),
        Some("series")
    );
    adapter.fronts = Some("series-upstream".to_owned());
    assert_eq!(
        curator
            .upstream_of(&adapter)
            .map(|one| one.service.as_str()),
        Some("series-upstream")
    );
}
