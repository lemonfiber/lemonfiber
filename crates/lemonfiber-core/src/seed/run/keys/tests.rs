use super::minted_key;
use crate::test_support::{
    a_minting_curator, env_without_password, first_party, stack_fillers, A_MINTED_KEY,
};

#[test]
fn a_minted_key_is_read_from_the_secret_store_and_absent_until_kept() {
    let env = env_without_password("keys-minted");
    let ctx = crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.to_path_buf()),
            ..crate::config::Settings::default()
        })
        .build();
    let fillers = stack_fillers(
        Vec::new(),
        &[a_minting_curator()],
        None,
        &first_party("series"),
    );
    let keeping = [crate::settings::Keeping {
        name: A_MINTED_KEY,
        value: "minted-key",
        from: crate::settings::Source::Operator,
        at: 1,
    }];
    let at = crate::settings::secrets_at(&ctx).unwrap_or_default();

    let mut read = Vec::new();
    if let Some(upstream) = fillers.service("series-upstream") {
        read.push(minted_key(&ctx, upstream));
        assert!(crate::settings::kept(
            &at,
            &keeping,
            &lemonfiber_fixtures::ports::Chance::cycling(),
            &|_| Ok(())
        )
        .is_ok());
        read.push(minted_key(&ctx, upstream));
    }

    assert_eq!(read, vec![None, Some("minted-key".to_owned())]);
}

#[test]
fn the_account_an_adapter_signs_in_as_is_never_read_as_its_upstreams_key() {
    let env = env_without_password("keys-account");
    let ctx = crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.to_path_buf()),
            ..crate::config::Settings::default()
        })
        .build();
    let mut signing_in = a_minting_curator();
    for placed in &mut signing_in.services {
        if placed.fronts.is_some() {
            placed.upstream.insert(
                lemonfiber_contract::adapter::UPSTREAM_USERNAME.to_owned(),
                "upstream-key".to_owned(),
            );
        }
    }
    let fillers = stack_fillers(Vec::new(), &[signing_in], None, &first_party("series"));
    let keeping = [crate::settings::Keeping {
        name: A_MINTED_KEY,
        value: "the-administrator",
        from: crate::settings::Source::Operator,
        at: 1,
    }];
    let at = crate::settings::secrets_at(&ctx).unwrap_or_default();
    assert!(crate::settings::kept(
        &at,
        &keeping,
        &lemonfiber_fixtures::ports::Chance::cycling(),
        &|_| Ok(())
    )
    .is_ok());

    let upstream = fillers.service("series-upstream");

    assert_eq!(
        upstream.map(|one| (one.minted.as_deref(), super::keyed(one))),
        Some((Some(A_MINTED_KEY), None))
    );
    assert_eq!(upstream.and_then(|one| minted_key(&ctx, one)), None);
}
