//! Proving a replacement credential before the one it replaces is discarded.
//!
//! Six of the settings an operator can change by name are a field of a credential this
//! product can test against a live service: the indexer's address and key, and the four
//! halves of a Usenet login plus whether it is reached over TLS. A replacement for one
//! of them is a paste, and a paste can be wrong — so the credential is assembled as it
//! *would* stand once the change is applied, and asked of the service, while the
//! credential in force is still the one on disk.
//!
//! The candidate is built from the file with the one setting overridden rather than from
//! the new value alone, because a credential is the whole login: a key proven against
//! the address it will actually be sent to is proven, and a key proven against the
//! address it happens to sit beside today is not.
//!
//! A credential that is not complete is not proven. Setting an address before there is a
//! key to send with it would otherwise be refused by an indexer answering about a key
//! nobody has given yet — which would make the first half of a two-part change
//! impossible to make.

use crate::config::{
    env::EnvFile, reads_as_on, INDEXER_APIKEY_KEY, INDEXER_URL_KEY, PROVIDER_HOST_KEY,
    PROVIDER_PASS_KEY, PROVIDER_PORT_KEY, PROVIDER_TLS_KEY, PROVIDER_USER_KEY,
};
use crate::validate::Credential;

/// What proving a replacement would take.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Proving {
    /// Nothing to prove: this setting is not part of a credential.
    Nothing,
    /// Part of one, but the credential it belongs to is still missing a half, so
    /// there is no login to put to a service yet.
    Incomplete,
    /// The credential as it would stand once the change is applied.
    Replacement(Box<Credential>),
    /// The replacement cannot be read as what the setting is for, so there is
    /// nothing to prove and nothing that could be applied.
    Unreadable(String),
}

/// What proving the change to `key` would take, over the file as it stands.
pub(super) fn wanted(file: &EnvFile, key: &str, value: &str) -> Proving {
    let held = |wanted: &str| -> String {
        if wanted == key {
            value.to_owned()
        } else {
            file.get(wanted).unwrap_or_default().to_owned()
        }
    };
    match key {
        INDEXER_URL_KEY | INDEXER_APIKEY_KEY => {
            indexer(&held(INDEXER_URL_KEY), &held(INDEXER_APIKEY_KEY))
        }
        PROVIDER_HOST_KEY | PROVIDER_PORT_KEY | PROVIDER_USER_KEY | PROVIDER_PASS_KEY
        | PROVIDER_TLS_KEY => usenet(
            &held(PROVIDER_HOST_KEY),
            &held(PROVIDER_PORT_KEY),
            &held(PROVIDER_USER_KEY),
            &held(PROVIDER_PASS_KEY),
            &held(PROVIDER_TLS_KEY),
        ),
        _ => Proving::Nothing,
    }
}

/// The indexer as it would stand, where both halves of it are there.
fn indexer(url: &str, key: &str) -> Proving {
    if url.trim().is_empty() || key.trim().is_empty() {
        return Proving::Incomplete;
    }
    Proving::Replacement(Box::new(Credential::Indexer {
        url: url.trim().to_owned(),
        key: key.trim().to_owned(),
    }))
}

/// The Usenet login as it would stand, where every half of it is there.
///
/// The port is the one field with a shape of its own. A value that is not a port
/// number cannot be dialled and cannot be corrected by the provider, so it is read
/// as unreadable rather than carried into a connection that would fail obscurely.
fn usenet(host: &str, port: &str, user: &str, pass: &str, tls: &str) -> Proving {
    if [host, user, pass].iter().any(|half| half.trim().is_empty()) {
        return Proving::Incomplete;
    }
    let port = port.trim();
    if port.is_empty() {
        return Proving::Incomplete;
    }
    let Ok(port) = port.parse::<u16>() else {
        return Proving::Unreadable(format!(
            "{PROVIDER_PORT_KEY} takes a port number from 1 to 65535, and {port:?} is not one"
        ));
    };
    Proving::Replacement(Box::new(Credential::Usenet {
        host: host.trim().to_owned(),
        port,
        secure: reads_as_on(tls),
        user: user.trim().to_owned(),
        pass: pass.to_owned(),
    }))
}

#[cfg(test)]
mod tests {
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
        let file = EnvFile::parse(
            "USENET_HOST=news.example.net\nUSENET_USER=someone\nUSENET_PASS=old-pass\n",
        );
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
}
