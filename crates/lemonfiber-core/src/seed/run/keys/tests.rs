use super::curator_key;
use crate::ports::filesystem::Beneath;
use crate::test_support::{
    a_minting_curator, env_without_password, first_party, stack_fillers, A_MINTED_KEY,
};

#[tokio::test]
async fn a_minted_key_is_read_from_the_secret_store_and_absent_until_kept() {
    let env = env_without_password("keys-minted");
    let ctx = crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(env),
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
        read.push(curator_key(&ctx, upstream).await);
        assert!(crate::settings::kept(&at, &keeping, &|_| Ok(())).is_ok());
        read.push(curator_key(&ctx, upstream).await);
    }

    assert_eq!(
        read,
        vec![Beneath::Absent, Beneath::Read("minted-key".to_owned())]
    );
}
