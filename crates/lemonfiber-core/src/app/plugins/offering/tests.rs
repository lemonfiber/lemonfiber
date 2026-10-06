use super::{acting, Consent, INSTALLING};

/// An offer of four parts, as an install names one.
fn standing() -> String {
    crate::agreement::parted(&[&["plugin"], &["writes"], &["contests"], &["a@b"]])
}

/// What the run is refused with, by code.
fn refused(result: &Result<bool, Box<crate::error::Problem>>) -> Option<&str> {
    result.as_ref().err().map(|problem| problem.code.as_str())
}

#[test]
fn no_offer_is_the_reading_and_acts_on_nothing() {
    let ctx = crate::test_support::a_context().build();
    let consent = Consent::default();
    assert!(matches!(
        acting(&ctx, &consent, "komga", &standing(), &INSTALLING, &["a@b"]),
        Ok(false)
    ));
}

#[test]
fn the_offer_standing_with_every_pair_approved_acts_and_a_rehearsal_does_not() {
    let consent = Consent {
        agreement: Some(standing()),
        approved: vec!["a@b".to_owned()],
    };
    let ctx = crate::test_support::a_context().build();
    assert!(matches!(
        acting(&ctx, &consent, "komga", &standing(), &INSTALLING, &["a@b"]),
        Ok(true)
    ));
    assert!(matches!(
        acting(
            &ctx.rehearsing(),
            &consent,
            "komga",
            &standing(),
            &INSTALLING,
            &["a@b"]
        ),
        Ok(false)
    ));
}

#[test]
fn an_offer_that_moved_is_refused_naming_only_the_part_that_did() {
    let ctx = crate::test_support::a_context().build();
    let consent = Consent {
        agreement: Some(crate::agreement::parted(&[
            &["plugin"],
            &["other writes"],
            &["contests"],
            &["a@b"],
        ])),
        approved: vec!["a@b".to_owned()],
    };
    let result = acting(&ctx, &consent, "komga", &standing(), &INSTALLING, &["a@b"]);
    assert_eq!(refused(&result), Some("PLUGIN-25"));
    let problem = result.err();
    assert_eq!(
        problem.as_ref().map(|problem| problem.amiss),
        Some(crate::agreement::MOVED_AMISS)
    );
    assert!(
        problem.is_some_and(|problem| problem.meaning.contains("what it would write")
            && !problem.meaning.contains("the plugin "))
    );
}

#[test]
fn a_pair_not_approved_is_refused_by_name() {
    let ctx = crate::test_support::a_context().build();
    let consent = Consent {
        agreement: Some(standing()),
        approved: Vec::new(),
    };
    let result = acting(&ctx, &consent, "komga", &standing(), &INSTALLING, &["a@b"]);
    assert_eq!(refused(&result), Some("PLUGIN-26"));
    assert!(result
        .err()
        .and_then(|problem| problem.remedies.first().and_then(|one| one.detail.clone()))
        .is_some_and(|detail| detail.contains("a@b")));
}

#[test]
fn an_approval_of_a_pair_nothing_carries_is_refused() {
    let ctx = crate::test_support::a_context().build();
    let consent = Consent {
        agreement: Some(standing()),
        approved: vec!["a@b".to_owned(), "token@elsewhere.example".to_owned()],
    };
    let result = acting(&ctx, &consent, "komga", &standing(), &INSTALLING, &["a@b"]);
    assert_eq!(refused(&result), Some("PLUGIN-26"));
    assert!(result
        .err()
        .is_some_and(|problem| problem.summary.contains("token@elsewhere.example")));
}

/// A pair to a service in the stack carries nothing off the machine and asks for no
/// approval, so an approval written for one names a pair nothing asks about.
#[test]
fn an_approval_of_a_pair_inside_the_stack_is_refused_as_one_nothing_asks_for() {
    let pair = |to: &str, outside: bool| crate::plugin::Pair {
        value: "token".to_owned(),
        origin: "stack-service".to_owned(),
        to: to.to_owned(),
        approval: outside.then(|| crate::plugin::approval("token", to)),
    };
    let recipes = [crate::plugin::Recipe {
        id: "adopt".to_owned(),
        title: "Adopt".to_owned(),
        why: "Held".to_owned(),
        steps: Vec::new(),
        pairs: vec![pair("komga", false), pair("meta.example.org", true)],
    }];
    let asked = crate::plugin::approvals(&recipes);
    assert_eq!(asked, ["token@meta.example.org"]);

    let ctx = crate::test_support::a_context().build();
    let consent = Consent {
        agreement: Some(standing()),
        approved: vec![
            "token@meta.example.org".to_owned(),
            "token@komga".to_owned(),
        ],
    };
    let result = acting(&ctx, &consent, "komga", &standing(), &INSTALLING, &asked);
    assert_eq!(refused(&result), Some("PLUGIN-26"));
    assert!(result
        .err()
        .is_some_and(|problem| problem.summary.contains("token@komga")));
}

/// An approval is the asker's own words, so the refusal that repeats one repeats it
/// with nothing in it a terminal would obey.
#[test]
fn a_stray_approval_is_repeated_with_nothing_a_terminal_obeys() {
    let ctx = crate::test_support::a_context().build();
    let consent = Consent {
        agreement: Some(standing()),
        approved: vec!["a@b".to_owned(), "token\u{1b}[2J@elsewhere".to_owned()],
    };
    let result = acting(&ctx, &consent, "komga", &standing(), &INSTALLING, &["a@b"]);
    assert!(result.err().is_some_and(|problem| {
        problem.summary.contains("token[2J@elsewhere") && !problem.summary.contains('\u{1b}')
    }));
}

/// The plugin, which a stranger writes, is sealed rather than checksummed; and it is
/// sealed over the bytes that were read, so a manifest read differently names another
/// offer.
#[test]
fn the_plugin_is_sealed_over_the_bytes_that_were_read() {
    let would = crate::test_support::an_installed("komga", Vec::new());
    let offer = super::installing("aaaa", &would, &[], &[]);
    let plugin = offer.split('-').next().unwrap_or_default();
    assert_eq!(plugin.len(), 32, "{offer}");
    let other = super::installing("bbbb", &would, &[], &[]);
    assert_ne!(other.split('-').next(), Some(plugin));
}
