//! What taking back the request service's credentials revokes and resets, and what it
//! leaves owed when the service refuses.

use super::*;

#[tokio::test]
async fn the_request_services_own_media_server_key_is_revoked() {
    let http = household(
        curator_gated(),
        requests::taken(),
        keys(true),
        media::taken(),
    );
    let (ctx, project) = taking("revoked", &http, false);
    let (state, _) = taken(&ctx, &gated(), &project, Baseline::new()).await;
    assert_eq!(state, Some(State::Wired));
    assert!(http.requests().iter().any(|asked| revoking(asked, MINTED)));
}

#[tokio::test]
async fn a_refused_revoke_is_said_and_left_for_the_next_run() {
    let http = household(
        curator_gated(),
        requests::taken(),
        keys(true),
        media::refusing(),
    );
    let (ctx, project) = taking("unrevoked", &http, false);
    let (state, _) = taken(&ctx, &gated(), &project, Baseline::new()).await;
    assert!(
        matches!(&state, Some(State::Failed { detail })
            if detail.starts_with("Seerr's own Jellyfin key could not be revoked: ")
                && detail.ends_with("It still opens Jellyfin; the next run revokes it.")),
        "{state:?}"
    );
}

#[tokio::test]
async fn a_refused_reset_is_said_and_stays_owed() {
    let http = household(
        curator_gated(),
        requests::taken(),
        keys(false),
        media::taken(),
    );
    let (ctx, project) = taking_with("refused", &http, false, false);
    let (state, baseline) = taken(&ctx, &gated(), &project, owing()).await;
    assert!(
        matches!(&state, Some(State::Failed { detail })
            if detail.starts_with("sonarr upstream-key could not be replaced: ")),
        "{state:?}"
    );
    assert_eq!(curator_key(&ctx).as_deref(), Some(OLD_KEY));
    assert_eq!(baseline.expected("seerr", OWED_CURATOR), Some("owed"));
}

/// The television curator's plugin with no secret it mints.
fn unminted() -> Option<crate::plugin::Installed> {
    TELEVISION.installed().map(|mut installed| {
        for secret in &mut installed.declared.secrets {
            secret.minted = false;
        }
        installed
    })
}

/// What this step comes to over `ctx` and `fillers` where the curator's key is owed: said
/// as one lemonfiber did not mint, and still owed.
async fn said_unminted(ctx: &Ctx, fillers: &crate::wiring::Fillers) {
    let (state, baseline) = taken_among(ctx, &gated(), fillers, owing()).await;
    assert!(
        matches!(&state, Some(State::Failed { detail })
            if detail.contains("lemonfiber replaces a key only where it minted it")),
        "{state:?}"
    );
    assert_eq!(baseline.expected("seerr", OWED_CURATOR), Some("owed"));
}

#[tokio::test]
async fn a_key_lemonfiber_did_not_mint_is_said_and_stays_owed() {
    let http = household(
        curator_gated(),
        requests::taken(),
        keys(false),
        media::taken(),
    );
    let (ctx, project) = taking("unminted", &http, false);
    let fillers = moving(
        gated(),
        &project,
        &[
            media::moved(),
            unminted(),
            MUSIC.installed(),
            REQUESTS.installed(),
        ],
    );
    said_unminted(&ctx, &fillers).await;
}

#[tokio::test]
async fn a_key_the_record_of_what_is_installed_says_was_not_minted_stays_owed() {
    let http = household(
        curator_gated(),
        requests::taken(),
        keys(false),
        media::taken(),
    );
    let (ctx, project) = taking("unrecorded-mint", &http, false);
    let kept = ctx
        .settings
        .env_file
        .as_deref()
        .map(|env| env.with_file_name(crate::config::paths::PLUGINS))
        .unwrap_or_default();
    let mut register =
        crate::plugin::Register::parse(&std::fs::read_to_string(&kept).unwrap_or_default())
            .unwrap_or_else(|_| crate::plugin::Register::empty());
    register.forget(TELEVISION.plugin);
    if let Some(installed) = unminted() {
        let _ = register.record(installed);
    }
    let _ = std::fs::write(&kept, register.to_json().unwrap_or_default());

    said_unminted(&ctx, &moved(gated(), &project)).await;
}

#[tokio::test]
async fn a_landed_reset_is_no_longer_owed() {
    let http = household(
        curator_gated(),
        requests::taken(),
        keys(false),
        media::taken(),
    );
    let (ctx, project) = taking("landed", &http, false);
    let (state, baseline) = taken(&ctx, &gated(), &project, owing()).await;
    assert_eq!(baseline.expected("seerr", OWED_CURATOR), None, "{state:?}");
    assert_ne!(curator_key(&ctx).as_deref(), Some(OLD_KEY));
    // Whatever copy could not be given the new key is named, in the rotation's words.
    if let Some(State::Failed { detail }) = &state {
        assert!(detail.contains("could not be given it"), "{detail}");
    }
}
