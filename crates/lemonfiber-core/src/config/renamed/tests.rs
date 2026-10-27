use super::{asked_now, earlier, RENAMED};

#[test]
fn each_key_kept_now_is_found_under_the_key_an_earlier_release_wrote() {
    for (was, now) in RENAMED {
        assert_eq!(earlier(now), Some(was), "{now}");
    }
    assert_eq!(earlier("DATA_ROOT"), None);
}

#[test]
fn a_credential_asked_for_by_an_earlier_name_is_asked_for_by_its_own() {
    assert_eq!(
        asked_now("Jellyfin Administrator Password").as_deref(),
        Some("media server administrator password")
    );
    assert_eq!(asked_now("sonarr_api_key"), None);
}
