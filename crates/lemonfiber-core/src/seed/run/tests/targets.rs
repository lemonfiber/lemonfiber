//! Which services a pass reaches, and where each keeps its configuration.

use super::*;

#[test]
fn the_project_directory_is_the_external_path_or_the_materialise_target() {
    // Any directory does: what is under test is which path is chosen, not what is
    // in it. `adapters` is named because the crate cannot compile without it — the
    // previous choice was a directory that later moved to its own crate, which broke
    // this at a distance with an error naming neither.
    static EMBEDDED: include_dir::Dir<'_> =
        include_dir::include_dir!("$CARGO_MANIFEST_DIR/src/config");
    assert_eq!(
        project_directory(&Source::External(std::path::Path::new("/srv/stack")), None).as_deref(),
        Some(std::path::Path::new("/srv/stack")),
        "an external stack is its own project root"
    );
    assert_eq!(
        project_directory(
            &Source::Embedded(&EMBEDDED),
            Some(std::path::Path::new("/opt/lemonfiber/stack"))
        )
        .as_deref(),
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
        "an embedded stack's root is wherever it was materialised"
    );
    assert_eq!(
        project_directory(&Source::Embedded(&EMBEDDED), None),
        None,
        "an embedded stack materialised nowhere has no root to read from"
    );
}

/// What the shipped stack's asks come to on this machine, with nothing installed.
fn shipped_fillers(project: Option<&std::path::Path>) -> crate::wiring::Fillers {
    crate::test_support::stack()
        .manifest()
        .map(|manifest| {
            crate::wiring::Fillers::of(&manifest, &[], &crate::wiring::Chosen::default(), project)
        })
        .unwrap_or_default()
}

/// The Usenet client's service, which its plugin is named after.
const USENET: &str = crate::test_support::usenet::PLUGIN;

/// The torrent client's service, which its plugin is named after.
const TORRENT: &str = crate::test_support::torrent::PLUGIN;

/// The shipped stack with `moved` moved into its first-party plugin, written at the
/// stack's root.
fn moved_fillers(moved: Option<crate::plugin::Installed>) -> crate::wiring::Fillers {
    crate::test_support::stack()
        .manifest()
        .map(|manifest| {
            crate::wiring::Fillers::trusting(
                &manifest,
                &moved.into_iter().collect::<Vec<_>>(),
                &crate::wiring::Chosen::default(),
                Some(stack_root()),
                crate::plugin::first_party::EMBEDDED,
            )
        })
        .unwrap_or_default()
}

/// The shipped stack with its Usenet client moved into its plugin.
fn moved_usenet_fillers() -> crate::wiring::Fillers {
    moved_fillers(crate::test_support::usenet::moved())
}

/// The shipped stack with its torrent client moved into its plugin.
fn moved_torrent_fillers() -> crate::wiring::Fillers {
    moved_fillers(crate::test_support::torrent::moved())
}

/// A Usenet client's key is the one lemonfiber minted for it, held against the moved
/// client it was minted for rather than against its kind or the stack's own client.
#[tokio::test]
async fn a_usenet_clients_key_is_held_against_the_client_it_was_minted_for() {
    let env = config_scratch("held-usenet-minted");
    let ctx = minting_usenet(seed_ctx(true, None, Some(env.to_path_buf())));

    let held = super::super::clients::held(&ctx, &moved_usenet_fillers());

    assert_eq!(
        held.of(&holder(Some(USENET), USENET)),
        Some(&Credential::ApiKey(MINTED_USENET_KEY.to_owned()))
    );
    assert!(held.of(&holder(None, USENET)).is_none());
}

/// No Usenet client's key is in hand until one is minted for it: not for a moved client
/// whose key the secret store does not hold yet, nor for the stack's own client, which
/// nothing mints a key for.
#[tokio::test]
async fn a_usenet_key_is_in_hand_only_once_minted() {
    let (unminted_env, unmoved_env) = (
        config_scratch("held-usenet-unminted"),
        config_scratch("held-usenet-unmoved"),
    );
    let unminted = seed_ctx(true, None, Some(unminted_env.to_path_buf()));
    let unmoved = minting_usenet(seed_ctx(true, None, Some(unmoved_env.to_path_buf())));

    let not_yet = super::super::clients::held(&unminted, &moved_usenet_fillers());
    let never = super::super::clients::held(&unmoved, &shipped_fillers(Some(stack_root())));

    assert!(
        !not_yet.keys.keys().any(|holder| holder.id == USENET),
        "held before it was minted"
    );
    assert!(
        !never.keys.keys().any(|holder| holder.id == USENET),
        "held for the stack's own client"
    );
}

/// A torrent client's account is the one lemonfiber minted for it, held against the moved
/// client it was minted for rather than against the stack's own client.
#[tokio::test]
async fn a_torrent_clients_account_is_held_against_the_client_it_was_minted_for() {
    let env = config_scratch("held-torrent-minted");
    let ctx = minting_torrent(seed_ctx(true, None, Some(env.to_path_buf())));

    let held = super::super::clients::held(&ctx, &moved_torrent_fillers());

    // Compared rather than printed: the values are credentials, and a failing
    // assertion prints its message into the run's log.
    let minted = Credential::UserPass {
        username: MINTED_TORRENT_USER.to_owned(),
        password: MINTED_TORRENT_PASSWORD.to_owned(),
    };
    assert!(held
        .of(&holder(Some(TORRENT), TORRENT))
        .is_some_and(|held| *held == minted));
    assert!(held.of(&holder(None, TORRENT)).is_none());
}

/// No torrent client's account is in hand until one is minted for it: not for a moved
/// client whose account the secret store does not hold yet, nor for the stack's own
/// client, whatever password is recorded for it.
#[tokio::test]
async fn a_torrent_account_is_in_hand_only_once_minted() {
    let (unminted_env, unmoved_env) = (
        config_scratch("held-torrent-unminted"),
        config_scratch("held-torrent-unmoved"),
    );
    let _ = store::set(
        &unmoved_env,
        crate::config::TORRENT_PASSWORD_KEY,
        "recorded-for-the-stacks",
    );
    let unminted = seed_ctx(true, None, Some(unminted_env.to_path_buf()));
    let unmoved = minting_torrent(seed_ctx(true, None, Some(unmoved_env.to_path_buf())));

    let not_yet = super::super::clients::held(&unminted, &moved_torrent_fillers());
    let never = super::super::clients::held(&unmoved, &shipped_fillers(Some(stack_root())));

    assert!(
        !not_yet.keys.keys().any(|holder| holder.id == TORRENT),
        "held before it was minted"
    );
    assert!(
        !never.keys.keys().any(|holder| holder.id == TORRENT),
        "held for the stack's own client"
    );
}

#[tokio::test]
async fn a_media_server_the_household_set_up_leaves_the_request_identity_to_them() {
    let env = config_scratch("targets-household-set-up");
    let _ = std::fs::create_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    let ctx = seed_ctx(true, None, Some(env.to_path_buf())).with_http(household(true, false));
    crate::test_support::media::moved_in(&ctx);
    REQUESTS.moved_in(&ctx);
    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    let identity = report
        .wirings
        .iter()
        .find(|wiring| wiring.connection == crate::seed::IDENTITY);
    assert!(identity.is_some_and(is_skipped), "{:?}", report.wirings);
}
