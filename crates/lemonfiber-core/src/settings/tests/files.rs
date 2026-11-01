use crate::app::Ctx;
use crate::config::env::quoted;
use crate::config::store;
use crate::plugin::{Holding, Variable};
use crate::test_support::{a_context, env_without_password, A_MINTED_KEY};

fn machine(name: &str) -> Ctx {
    let env = env_without_password(&format!("files-{name}"));
    a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.clone()),
            stack_dir: Some(env.with_file_name("data").join("stack")),
            ..crate::config::Settings::default()
        })
        .build()
}

fn variable(name: &str, holding: Holding) -> Variable {
    Variable {
        name: name.to_owned(),
        holding,
    }
}

fn kept(ctx: &Ctx, name: &str, value: &str) {
    let at = crate::settings::secrets_at(ctx).unwrap_or_default();
    let keeping = [crate::settings::Keeping {
        name,
        value,
        from: crate::settings::Source::Operator,
        at: 1,
    }];
    assert!(crate::settings::kept(
        &at,
        &keeping,
        &lemonfiber_fixtures::ports::Chance::cycling(),
        &|_| Ok(())
    )
    .is_ok());
}

#[test]
fn every_value_reads_back_as_exactly_itself_so_compose_substitutes_nothing() {
    let ctx = machine("quoted");
    let awkward = r#"a$B${C}"d\e'f"#;
    kept(&ctx, "komga:token", awkward);
    kept(&ctx, "komga:plain", "plain-value");
    let text = crate::settings::text(
        &ctx,
        &[
            variable("TOKEN", Holding::Store("komga:token".to_owned())),
            variable("PLAIN", Holding::Store("komga:plain".to_owned())),
        ],
    )
    .unwrap_or_default();
    let read: Vec<(String, String)> = text
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, raw)| (name.to_owned(), quoted::read(raw)))
        .collect();
    assert_eq!(
        read,
        [
            ("TOKEN".to_owned(), awkward.to_owned()),
            ("PLAIN".to_owned(), "plain-value".to_owned())
        ]
    );
    assert!(!text.replace("\\$", "").contains('$'), "{text}");
}

#[test]
fn a_secret_nothing_holds_or_that_may_not_cross_is_left_out() {
    let ctx = machine("unheld");
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    assert!(store::set(&env, "SONARR_API_KEY", "abc").is_ok());
    let text = crate::settings::text(
        &ctx,
        &[
            variable("MISSING", Holding::Store("komga:missing".to_owned())),
            variable("WITHHELD", Holding::Withheld),
            variable("CROSSED", Holding::Crossing("SONARR_API_KEY".to_owned())),
        ],
    );
    assert_eq!(text.ok().as_deref(), Some("CROSSED=abc\n"));
    let nothing = crate::settings::text(
        &ctx,
        &[variable(
            "MISSING",
            Holding::Store("komga:missing".to_owned()),
        )],
    );
    assert_eq!(nothing.ok().as_deref(), Some(""));
}

#[test]
fn a_stack_credential_does_not_cross_to_a_plugin_nobody_vouched_for() {
    let ctx = machine("crossing");
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok();
    let installed = crate::test_support::an_installed(
        "comics",
        vec![crate::test_support::a_placed("comics-reader", &[], None)],
    );
    let resolved =
        crate::settings::Resolved::of(&ctx, manifest.as_ref(), std::slice::from_ref(&installed));
    assert_eq!(
        resolved.crossing("comics", "sonarr"),
        Some(Holding::Withheld)
    );
    assert_eq!(resolved.crossing("comics", "comics-reader"), None);
    assert_eq!(resolved.crossing("comics", "nothing-here"), None);
}

#[test]
fn a_minted_key_crosses_by_its_secret_name_only_to_a_plugin_granted_it() {
    let ctx = machine("minted-crossing");
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok();
    let finder = |granted: &[&str]| {
        let mut finder = crate::test_support::an_installed(
            "finder",
            vec![crate::test_support::a_placed("finder", &[], None)],
        );
        finder.granted = granted
            .iter()
            .map(|credential| crate::plugin::Grant {
                credential: (*credential).to_owned(),
                of: "series-upstream".to_owned(),
                serves: "library.curate".to_owned(),
                at: String::new(),
            })
            .collect();
        finder
    };
    let crossing = |finder: crate::plugin::Installed| {
        crate::settings::Resolved::of(
            &ctx,
            manifest.as_ref(),
            &[crate::test_support::a_minting_curator(), finder],
        )
        .crossing("finder", "series-upstream")
    };

    assert_eq!(crossing(finder(&[])), Some(Holding::Withheld));
    assert_eq!(
        crossing(finder(&[A_MINTED_KEY])),
        Some(Holding::Store(A_MINTED_KEY.to_owned()))
    );
}

#[test]
fn a_plugin_secret_is_filed_only_where_the_setting_it_needs_holds_what_it_lists() {
    let ctx = machine("needing");
    let mut placed = crate::test_support::a_placed("guard", &[], None);
    placed.fields.env = toml::from_str(
        r#"
VPN_TYPE = { setting = "guard:vpn-type", default = "wireguard", one_of = ["wireguard", "openvpn"] }
OPENVPN_USER = { secret = "openvpn-user" }
"#,
    )
    .unwrap_or_default();
    let mut installed = crate::test_support::an_installed("guard", vec![placed]);
    installed.declared.secrets = vec![crate::plugin::Secret {
        id: "openvpn-user".to_owned(),
        of: "guard".to_owned(),
        why: "Signs in to the provider over OpenVPN.".to_owned(),
        minted: false,
        serves: None,
        was: None,
        needs: toml::from_str(r#""guard:vpn-type" = ["openvpn"]"#).unwrap_or_default(),
        needed_to_start: false,
        lapses_s: None,
        captured: false,
    }];
    let installed = [installed];
    let named = |resolved: &crate::settings::Resolved| -> Vec<String> {
        crate::settings::files::every_file(None, &installed, resolved)
            .into_iter()
            .flat_map(|(_, _, variables)| variables)
            .map(|variable| variable.name)
            .collect()
    };
    let resolved = crate::settings::Resolved::of(&ctx, None, &installed);
    assert!(named(&resolved).is_empty());
    assert_eq!(
        named(&resolved.changed(&[("guard:vpn-type", Some("openvpn"))].into())),
        ["OPENVPN_USER"]
    );
}
