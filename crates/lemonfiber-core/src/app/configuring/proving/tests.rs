use super::{wanted, Proving};
use crate::config::{
    env::EnvFile, INDEXER_APIKEY_KEY, INDEXER_URL_KEY, PROVIDER_PASS_KEY, PROVIDER_PORT_KEY,
    PROVIDER_TLS_KEY,
};
use crate::validate::Credential;

/// A file holding a complete indexer and a complete Usenet login.
fn configured() -> EnvFile {
    EnvFile::parse(
        "INDEXER_URL=https://indexer.example/api\nINDEXER_APIKEY=old-key\n\
         USENET_HOST=news.example.net\nUSENET_PORT=563\nUSENET_USER=someone\n\
         USENET_PASS=old-pass\nUSENET_TLS=on\n",
    )
}

#[test]
fn a_setting_that_is_no_part_of_a_credential_has_nothing_to_prove() {
    assert_eq!(
        wanted(&configured(), "DATA_ROOT", "/srv/new"),
        Proving::Nothing
    );
}

/// The key is put to the address it will actually be sent to, which is the
/// address in the file rather than one remembered from somewhere else.
#[test]
fn a_replacement_key_is_proven_against_the_address_it_will_be_sent_to() {
    let proving = wanted(&configured(), INDEXER_APIKEY_KEY, "new-key");
    assert_eq!(
        proving,
        Proving::Replacement(Box::new(Credential::Indexer {
            url: "https://indexer.example/api".to_owned(),
            key: "new-key".to_owned(),
        }))
    );
}

#[test]
fn a_replacement_address_is_proven_with_the_key_it_will_carry() {
    let proving = wanted(&configured(), INDEXER_URL_KEY, "https://other.example/api");
    assert_eq!(
        proving,
        Proving::Replacement(Box::new(Credential::Indexer {
            url: "https://other.example/api".to_owned(),
            key: "old-key".to_owned(),
        }))
    );
}

/// An address given before there is a key to send with it is half a credential,
/// and a service asked about half of one would refuse the half that is right.
#[test]
fn half_a_credential_is_not_put_to_a_service_at_all() {
    let file = EnvFile::parse("INDEXER_APIKEY=\n");
    let proving = wanted(&file, INDEXER_URL_KEY, "https://indexer.example/api");
    assert_eq!(proving, Proving::Incomplete);
}

#[test]
fn a_replacement_password_is_proven_against_the_whole_login() {
    let proving = wanted(&configured(), PROVIDER_PASS_KEY, "new-pass");
    assert_eq!(
        proving,
        Proving::Replacement(Box::new(Credential::Usenet {
            host: "news.example.net".to_owned(),
            port: 563,
            secure: true,
            user: "someone".to_owned(),
            pass: "new-pass".to_owned(),
        }))
    );
}

#[test]
fn turning_tls_off_is_proven_as_the_plaintext_login_it_would_become() {
    let proving = wanted(&configured(), PROVIDER_TLS_KEY, "off");
    assert!(
        matches!(&proving, Proving::Replacement(credential)
            if matches!(**credential, Credential::Usenet { secure: false, .. })),
        "{proving:?}"
    );
}

#[test]
fn a_login_missing_a_half_is_not_put_to_a_provider_either() {
    let file = EnvFile::parse("USENET_HOST=news.example.net\nUSENET_PORT=563\n");
    assert_eq!(
        wanted(&file, PROVIDER_PASS_KEY, "a-pass"),
        Proving::Incomplete
    );
}

/// A login whose port has never been recorded is incomplete rather than
/// unreadable: nothing has been mistyped, there is simply nothing there yet.
#[test]
fn a_login_with_no_port_recorded_is_incomplete_rather_than_unreadable() {
    let file =
        EnvFile::parse("USENET_HOST=news.example.net\nUSENET_USER=someone\nUSENET_PASS=old-pass\n");
    assert_eq!(
        wanted(&file, PROVIDER_PASS_KEY, "new-pass"),
        Proving::Incomplete
    );
}

#[test]
fn a_port_that_is_not_a_port_number_is_unreadable_and_says_so() {
    let proving = wanted(&configured(), PROVIDER_PORT_KEY, "five-six-three");
    assert!(
        matches!(&proving, Proving::Unreadable(why)
            if why.contains(PROVIDER_PORT_KEY) && why.contains("five-six-three")),
        "{proving:?}"
    );
}

#[test]
fn a_port_outside_the_range_a_port_has_is_unreadable_too() {
    let proving = wanted(&configured(), PROVIDER_PORT_KEY, "70000");
    assert!(matches!(proving, Proving::Unreadable(_)), "{proving:?}");
}
