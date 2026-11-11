use super::{ran, ReleasesCheck, NONE};
use crate::doctor::credentials::Target;
use crate::doctor::narrowing::Narrowing;
use crate::doctor::{Disruptive, Verdict};
use crate::ports::media::TitleKind;

/// The curator `id`, filing `kind`, not yet asked through any adapter.
fn filing(id: &str, kind: TitleKind) -> Target {
    Target {
        id: id.to_owned(),
        name: id.to_owned(),
        kind: Some(kind),
        asked: None,
    }
}

/// The television curator, filing video by resolution.
fn television() -> Target {
    filing("sonarr", TitleKind::Tv)
}

#[tokio::test]
async fn music_is_searched_at_its_chosen_quality_and_books_which_have_none_are_not() {
    let check = ReleasesCheck::new(
        vec![
            filing("lidarr", TitleKind::Music),
            filing("bindery", TitleKind::Books),
        ],
        true,
    );

    let findings = ran(&check).await;

    let checked: Vec<(&str, &str)> = findings
        .iter()
        .map(|found| (found.check.as_str(), found.title.as_str()))
        .collect();
    assert_eq!(
        checked,
        [(
            "services.releases.lidarr",
            "Releases for music at the chosen quality"
        )]
    );
}

#[tokio::test]
async fn a_stack_filing_books_alone_has_no_releases_to_check() {
    let check = ReleasesCheck::new(vec![filing("bindery", TitleKind::Books)], true);

    let findings = ran(&check).await;

    let verdicts: Vec<&Verdict> = findings.iter().map(|found| &found.verdict).collect();
    assert!(
        matches!(verdicts.as_slice(), [Verdict::Skipped { .. }]),
        "{verdicts:?}"
    );
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
