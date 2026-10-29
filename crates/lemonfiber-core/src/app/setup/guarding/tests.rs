use std::collections::BTreeMap;

use lemonfiber_plugin::Manifest;

use super::{checking, Asking, Guard};
use crate::error::codes::setting::OUTSIDE_ITS_CHOICES;
use crate::error::codes::setup::GUARD_UNANSWERED;
use crate::model::GuardQuestion;
use crate::plugin::Installed;
use crate::wizard::Guarded;

const GUARD: &str = r#"
schema_version = 1

[plugin]
id          = "guard"
name        = "Guard"
version     = "1.0.0"
description = "Carries the downloads through a tunnel"
without_it  = "Downloads over the open line"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "tunnel"
name        = "Tunnel"
image       = "example.invalid/tunnel"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.0.0"
criticality = "important"
config_path = "/config"
provides    = ["network.egress-guard"]

[service.env]
PROVIDER      = { setting = "guard:provider", default = "mullvad", one_of = [
  "mullvad",
  { values = ["ivpn"], needs = { "guard:vpn-type" = ["wireguard"] } },
] }
VPN_TYPE      = { setting = "guard:vpn-type", default = "wireguard", one_of = ["wireguard", "openvpn"] }
COUNTRY       = { setting = "guard:country", default = "Netherlands" }
ACCOUNT       = { setting = "guard:account", default = "", recorded_with = "0000", needs = { "guard:vpn-type" = ["openvpn"] } }
WIREGUARD_KEY = { secret = "wireguard-key" }
OPENVPN_USER  = { secret = "openvpn-user" }
CERTIFICATE   = { secret = "certificate" }

[[secret]]
id            = "wireguard-key"
of            = "tunnel"
why           = "Dials the tunnel"
recorded_with = "minted"
needs         = { "guard:vpn-type" = ["wireguard"] }

[[secret]]
id            = "openvpn-user"
of            = "tunnel"
why           = "Signs in over OpenVPN"
recorded_with = "minted"
needs         = { "guard:vpn-type" = ["openvpn"] }

[[secret]]
id  = "certificate"
of  = "tunnel"
why = "Pins the provider's servers"
"#;

/// The guard [`GUARD`] declares, installed.
pub(in crate::app::setup) fn guard() -> Guard {
    guard_of(GUARD)
}

/// The guard the plugin `text` declares, installed.
fn guard_of(text: &str) -> Guard {
    let installed: Vec<Installed> = Manifest::from_toml(text)
        .iter()
        .map(Installed::of)
        .collect();
    let Some(guard) = Guard::of(&stack(), &installed, "tunnel") else {
        unreachable!("the tunnel is installed");
    };
    guard
}

/// The stack this build carries.
fn stack() -> lemonfiber_manifest::Manifest {
    let manifest = crate::test_support::a_context().build().stack.manifest();
    let Ok(manifest) = manifest else {
        unreachable!("the stack reads");
    };
    manifest
}

/// An answer giving `settings` and `secrets`.
fn given(settings: &[(&str, &str)], secrets: &[(&str, &str)]) -> Guarded {
    let owned = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    };
    Guarded {
        settings: owned(settings),
        secrets: owned(secrets),
    }
}

/// What a check of `given` against [`guard`] is refused with: its code and what it says.
fn refused(given: &Guarded) -> Option<(crate::error::Code, String)> {
    guard().taken(given).err().map(|problem| {
        (
            problem.code,
            format!("{} {}", problem.summary, problem.meaning),
        )
    })
}

#[test]
fn each_choice_is_asked_after_what_its_values_need_then_what_the_guard_needs_to_start() {
    let named: Vec<(String, String)> = guard()
        .questions()
        .into_iter()
        .map(|question| match question {
            GuardQuestion::Choice { setting, .. } => ("choice".to_owned(), setting),
            GuardQuestion::Value { setting, .. } => ("value".to_owned(), setting),
            GuardQuestion::Secret { secret, .. } => ("secret".to_owned(), secret),
        })
        .collect();

    assert_eq!(
        named,
        vec![
            ("choice".to_owned(), "guard:vpn-type".to_owned()),
            ("choice".to_owned(), "guard:provider".to_owned()),
            ("value".to_owned(), "guard:account".to_owned()),
            ("secret".to_owned(), "guard:wireguard-key".to_owned()),
            ("secret".to_owned(), "guard:openvpn-user".to_owned()),
        ]
    );
    let provider = guard()
        .questions()
        .into_iter()
        .find_map(|question| match question {
            GuardQuestion::Choice {
                setting,
                default,
                choices,
            } if setting == "guard:provider" => Some((default, choices)),
            _ => None,
        });
    assert!(
        provider.is_some_and(|(default, choices)| default.as_deref() == Some("mullvad")
            && choices
                .iter()
                .any(|one| one.value == "ivpn" && one.open && !one.needs.is_empty()))
    );
}

#[test]
fn a_walk_offers_only_what_the_answers_before_it_open_and_asks_only_what_they_need() {
    let mut asked = Vec::new();
    let given = guard().asked(&mut |asking| {
        asked.push(asking.clone().into_owned_name());
        match asking {
            Asking::Choice { setting, .. } if *setting == "guard:vpn-type" => {
                Some("openvpn".to_owned())
            }
            Asking::Choice { offered, .. } => {
                assert_eq!(offered, &vec!["mullvad".to_owned()]);
                None
            }
            Asking::Value { .. } => Some("account-7".to_owned()),
            Asking::Secret { why, .. } => {
                assert_eq!(*why, "Signs in over OpenVPN");
                Some("someone".to_owned())
            }
        }
    });

    assert_eq!(
        asked,
        vec![
            "guard:vpn-type",
            "guard:provider",
            "guard:account",
            "guard:openvpn-user"
        ]
    );
    assert_eq!(
        given,
        self::given(
            &[
                ("guard:account", "account-7"),
                ("guard:vpn-type", "openvpn")
            ],
            &[("guard:openvpn-user", "someone")]
        )
    );
}

#[test]
fn an_answer_giving_everything_the_guard_asks_and_nothing_else_is_taken() {
    assert!(guard()
        .taken(&given(
            &[("guard:vpn-type", "wireguard"), ("guard:provider", "ivpn")],
            &[("guard:wireguard-key", "a-key")],
        ))
        .is_ok());
    assert!(guard()
        .taken(&given(&[], &[("guard:wireguard-key", "a-key")]))
        .is_ok());
}

#[test]
fn a_value_outside_what_the_answers_before_it_open_is_refused_naming_its_choices() {
    let outside = refused(&given(
        &[("guard:vpn-type", "openvpn"), ("guard:provider", "ivpn")],
        &[("guard:openvpn-user", "someone")],
    ));

    assert!(
        outside.is_some_and(|(code, said)| code == OUTSIDE_ITS_CHOICES
            && said.contains("guard:provider")
            && said.contains("mullvad"))
    );
}

#[test]
fn what_the_guard_needs_and_was_not_given_is_named_and_never_its_value() {
    let missing = refused(&given(
        &[("guard:vpn-type", "openvpn")],
        &[("guard:openvpn-user", "")],
    ));

    assert!(
        missing
            .as_ref()
            .is_some_and(|(code, said)| *code == GUARD_UNANSWERED
                && said.contains("guard:account")
                && said.contains("guard:openvpn-user")),
        "{missing:?}"
    );
}

#[test]
fn a_choice_left_unanswered_whose_default_the_answers_close_is_named_as_left_out() {
    let defaulted = guard_of(&GUARD.replace(
        r#"default = "mullvad", one_of"#,
        r#"default = "ivpn", one_of"#,
    ));

    let missing = defaulted.taken(&given(
        &[
            ("guard:vpn-type", "openvpn"),
            ("guard:account", "account-7"),
        ],
        &[("guard:openvpn-user", "someone")],
    ));

    assert!(missing
        .is_err_and(|problem| problem.code == GUARD_UNANSWERED
            && problem.meaning.contains("guard:provider")));
}

#[test]
fn a_name_the_guard_does_not_ask_for_under_the_answers_given_is_refused() {
    let dialled = "nobody-may-read-this";
    let foreign = refused(&given(
        &[("guard:country", "Sweden"), ("indexer:url", "elsewhere")],
        &[
            ("guard:wireguard-key", dialled),
            ("guard:openvpn-user", "someone"),
            ("guard:certificate", "pinned"),
        ],
    ));

    assert!(
        foreign
            .as_ref()
            .is_some_and(|(code, said)| *code == GUARD_UNANSWERED
                && [
                    "guard:country",
                    "indexer:url",
                    "guard:openvpn-user",
                    "guard:certificate"
                ]
                .iter()
                .all(|name| said.contains(name))
                && !said.contains(dialled)
                && !said.contains("someone")),
        "{foreign:?}"
    );
}

#[test]
fn with_no_guard_to_ask_only_an_answer_that_gives_nothing_is_taken() {
    assert!(checking(None, &Guarded::default()).is_ok());
    let refused = checking(None, &given(&[], &[("indexer:key", "theirs")]));
    assert!(
        refused.is_err_and(|problem| problem.code == GUARD_UNANSWERED
            && problem.meaning.contains("indexer:key")
            && !problem.meaning.contains("theirs"))
    );
    assert!(checking(
        Some(&guard()),
        &given(&[("guard:vpn-type", "openvpn")], &[])
    )
    .is_err());
}

#[test]
fn the_guard_takes_what_it_asks_for_and_nothing_left_blank() {
    let taken = guard().taken(&given(
        &[("guard:vpn-type", "wireguard"), ("guard:account", "")],
        &[
            ("guard:wireguard-key", "dialled"),
            ("guard:openvpn-user", ""),
        ],
    ));

    assert!(taken.is_ok_and(|taken| taken
        == given(
            &[("guard:vpn-type", "wireguard")],
            &[("guard:wireguard-key", "dialled")]
        )));
    assert!(checking(None, &given(&[("guard:vpn-type", "")], &[]))
        .is_ok_and(|taken| taken == Guarded::default()));
}

#[test]
fn choices_that_need_each_other_are_asked_by_name_rather_than_never() {
    let circled = GUARD.replace(
        r#"VPN_TYPE      = { setting = "guard:vpn-type", default = "wireguard", one_of = ["wireguard", "openvpn"] }"#,
        r#"VPN_TYPE      = { setting = "guard:vpn-type", default = "wireguard", one_of = ["wireguard", { values = ["openvpn"], needs = { "guard:provider" = ["mullvad"] } }] }"#,
    );
    let chosen: Vec<String> = guard_of(&circled)
        .questions()
        .into_iter()
        .filter_map(|question| match question {
            GuardQuestion::Choice { setting, .. } => Some(setting),
            _ => None,
        })
        .collect();

    assert_eq!(chosen, vec!["guard:provider", "guard:vpn-type"]);
}

#[test]
fn a_service_that_is_not_there_is_no_guard() {
    assert!(Guard::of(&stack(), &[], "tunnel").is_none());
}

impl Asking<'_> {
    /// The setting or the secret this asks for.
    fn into_owned_name(self) -> String {
        match self {
            Self::Choice { setting, .. } | Self::Value { setting } => setting.to_owned(),
            Self::Secret { secret, .. } => secret.to_owned(),
        }
    }
}

mod answered;
