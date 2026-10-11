use std::time::{Duration, SystemTime};

use lemonfiber_fixtures::ports::Chance;

use super::{claim_shaped, Claim, Claimed, Pending, CLAIM_PREFIX};
use crate::keys::{shaped, Purpose};

fn at(seconds: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
}

fn pending(lapses: &str) -> Pending {
    Pending {
        digest: "d".repeat(64),
        lapses: lapses.to_owned(),
    }
}

#[test]
fn a_claim_is_shaped_as_one_and_never_as_a_secret() {
    let claim = Claim::mint(&Chance::cycling()).map(|claim| claim.as_str().to_owned());
    let claim = claim.unwrap_or_default();
    assert!(claim.starts_with(CLAIM_PREFIX), "{claim}");
    assert!(claim_shaped(&claim));
    assert!(!shaped(&claim));
    assert!(!claim_shaped("lfc_short"));
    assert!(!claim_shaped(&claim.replace(CLAIM_PREFIX, "lfk_")));
}

#[test]
fn a_claim_is_not_minted_from_too_few_bytes() {
    let short = lemonfiber_fixtures::support::FixedRandom(Some(vec![1; 8]));
    assert!(Claim::mint(&short).is_none());
    let none = lemonfiber_fixtures::support::FixedRandom(None);
    assert!(Claim::mint(&none).is_none());
}

#[test]
fn a_claim_and_its_link_are_never_printed() {
    let claim = Claim::mint(&Chance::cycling());
    let said = format!("{claim:?}");
    assert!(!said.contains(CLAIM_PREFIX), "{said}");
    let claimed = Claimed {
        name: "ha".to_owned(),
        scope: "read".to_owned(),
        purpose: Purpose::HomeAssistant,
        link: "lemonfiber://key?claim=lfc_secretish".to_owned(),
        lapses: "2026-10-06T00:10:00".to_owned(),
    };
    let said = format!("{claimed:?}");
    assert!(!said.contains("lfc_secretish"), "{said}");
    assert!(said.contains("2026-10-06T00:10:00"), "{said}");
    let said = format!("{:?}", pending("2026-10-06T00:10:00"));
    assert!(!said.contains(&"d".repeat(64)), "{said}");
}

#[test]
fn a_claim_lapses_at_its_moment_and_not_before() {
    let lapses = pending("2026-10-06T00:10:00");
    let moment = 1_791_245_400;
    assert!(!lapses.lapsed(at(moment - 1)));
    assert!(lapses.lapsed(at(moment)));
    assert!(lapses.lapsed(at(moment + 1)));
}

#[test]
fn a_lapse_nobody_can_read_has_lapsed() {
    assert!(pending("soon").lapsed(at(0)));
}
