use super::{ran, ReleasesCheck, NONE};
use crate::doctor::credentials::Target;
use crate::doctor::narrowing::Narrowing;
use crate::doctor::{Disruptive, Verdict};

/// The television curator, filing video by resolution.
fn television() -> Target {
    Target {
        id: "sonarr".to_owned(),
        name: "Sonarr".to_owned(),
        kind: Some(crate::ports::media::Kind::Tv),
        asked: None,
    }
}

#[tokio::test]
async fn a_search_not_asked_for_says_what_it_spends_and_how_to_ask_for_it() {
    let check = ReleasesCheck::new(vec![television()], false);

    let findings = ran(&check).await;

    let [finding] = findings.as_slice() else {
        unreachable!("one finding was not reported: {findings:?}");
    };
    assert!(matches!(finding.verdict, Verdict::Unverified { .. }));
    assert_eq!(
        finding.disruptive,
        Some(Disruptive {
            disturbs: "one real search per service against the indexers, counted towards the \
                       daily cap they hold the operator to"
                .to_owned(),
            bounded: Some(crate::doctor::CHECK_BUDGET.as_secs()),
            only: NONE.to_owned(),
        })
    );
    let only = Narrowing::parse(NONE);
    assert!(
        only.as_ref().is_some_and(|only| only.keeps(finding)),
        "{only:?}"
    );
    assert!(
        matches!(&finding.verdict, Verdict::Unverified { remedy, .. }
            if remedy.detail.as_deref() == Some("lemonfiber doctor --only services.releases --disruptive")),
        "{finding:?}"
    );
}
