use std::process::Command;

use lemonfiber_fixtures::scratch::Scratch;

use super::{kept, kept_or_made, made_for, path, read, written, Unkept, INTERNAL};

/// What `openssl` says of the PKCS#12 at `at`, opened with the empty password, or why it
/// would not open it.
fn opened(at: &std::path::Path, asked: &[&str]) -> Result<String, String> {
    let ran = Command::new("openssl")
        .args(["pkcs12", "-in"])
        .arg(at)
        .args(["-password", "pass:"])
        .args(asked)
        .output()
        .map_err(|why| format!("openssl could not be run: {why}"))?;
    if ran.status.success() {
        Ok(format!(
            "{}{}",
            String::from_utf8_lossy(&ran.stdout),
            String::from_utf8_lossy(&ran.stderr)
        ))
    } else {
        Err(String::from_utf8_lossy(&ran.stderr).into_owned())
    }
}

/// The SHA-256 fingerprint `openssl` gives the certificate in `pem`, lower-case hex.
fn fingerprinted(pem: &str) -> Result<String, String> {
    use std::io::Write as _;
    let mut child = Command::new("openssl")
        .args(["x509", "-noout", "-fingerprint", "-sha256"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|why| format!("openssl could not be run: {why}"))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "openssl took no input".to_owned())?
        .write_all(pem.as_bytes())
        .map_err(|why| why.to_string())?;
    let done = child.wait_with_output().map_err(|why| why.to_string())?;
    let said = String::from_utf8_lossy(&done.stdout);
    let (_, digest) = said
        .trim()
        .split_once('=')
        .ok_or_else(|| format!("openssl said {said}"))?;
    Ok(digest.replace(':', "").to_lowercase())
}

#[test]
fn the_written_pkcs12_opens_with_the_empty_password_and_holds_the_certificate_pinned() {
    let config = Scratch::new("internal-opened");
    let made = made_for("jellyfin");
    let Ok(made) = made else {
        unreachable!("a certificate is made: {made:?}");
    };
    assert_eq!(written(&config, &made), Ok(()));
    let at = path(&config);
    assert!(at.ends_with(INTERNAL));

    let certificate = opened(&at, &["-nokeys", "-clcerts"]);
    assert!(
        certificate
            .as_ref()
            .is_ok_and(|pem| pem.contains("BEGIN CERTIFICATE")),
        "{certificate:?}"
    );
    assert_eq!(
        certificate.and_then(|pem| fingerprinted(&pem)),
        Ok(made.fingerprint.clone()),
        "the digest a holder is handed is the certificate the file presents"
    );
    let key = opened(&at, &["-nocerts", "-nodes"]);
    assert!(
        key.as_ref().is_ok_and(|pem| pem.contains("PRIVATE KEY")),
        "{key:?}"
    );
    let named = opened(&at, &["-nokeys", "-clcerts", "-info"]);
    assert!(
        named
            .as_ref()
            .is_ok_and(|said| said.contains("CN=jellyfin") && said.contains("PBES2")),
        "{named:?}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            std::fs::metadata(&at)
                .map(|file| file.permissions().mode() & 0o777)
                .ok(),
            Some(0o600)
        );
    }
}

#[test]
fn what_is_kept_is_read_back_and_made_once() {
    let config = Scratch::new("internal-kept");
    assert_eq!(kept(&config), Ok(None));
    let first = kept_or_made(&config, "jellyfin");
    let again = kept_or_made(&config, "jellyfin");
    assert!(first.is_ok(), "{first:?}");
    assert_eq!(first, again, "a kept certificate is presented unchanged");
    assert_eq!(kept(&config), first.map(Some));
    let other = made_for("jellyfin").map(|made| made.fingerprint);
    assert_ne!(
        other,
        again.map(|kept| kept.fingerprint),
        "each made certificate is its own"
    );
}

#[test]
fn what_is_not_a_pkcs12_is_refused_and_never_made_over() {
    let config = Scratch::new("internal-unreadable");
    let _ = std::fs::create_dir_all(path(&config).parent().unwrap_or(&config));
    let _ = std::fs::write(path(&config), b"not a pkcs12");
    assert!(matches!(kept(&config), Err(Unkept::Unreadable(_))));
    assert!(matches!(
        kept_or_made(&config, "jellyfin"),
        Err(Unkept::Unreadable(_))
    ));
    assert_eq!(
        std::fs::read(path(&config)).ok(),
        Some(b"not a pkcs12".to_vec())
    );
    assert!(matches!(read(b""), Err(Unkept::Unreadable(_))));
}

#[test]
fn a_store_holding_no_key_with_its_certificate_is_refused() {
    let Ok(made) = made_for("jellyfin") else {
        unreachable!("a certificate is made");
    };
    let mut store = p12_keystore::KeyStore::new();
    let held = p12_keystore::KeyStore::from_pkcs12(
        made.pkcs12(),
        "",
        p12_keystore::Pkcs12ImportPolicy::Strict,
    );
    let certificate = held.ok().and_then(|held| {
        held.private_key_chain()
            .and_then(|(_, chain)| chain.certs().first().cloned())
    });
    let Some(certificate) = certificate else {
        unreachable!("the made store holds its certificate");
    };
    store.add_entry(
        "alone",
        p12_keystore::KeyStoreEntry::Certificate(certificate),
    );
    let bytes = store.writer("").write().unwrap_or_default();
    assert!(matches!(read(&bytes), Err(Unkept::Unreadable(why)) if why.contains("no certificate")));
}

#[cfg(unix)]
#[test]
fn a_certificate_that_cannot_be_kept_where_it_is_asked_says_so() {
    use std::os::unix::fs::PermissionsExt as _;
    let config = Scratch::new("internal-unwritable");
    let _ = std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o500));
    let refused = kept_or_made(&config, "jellyfin");
    let unread = {
        let _ = std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o000));
        kept(&config)
    };
    let _ = std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o700));
    assert!(matches!(refused, Err(Unkept::Unmade(_))), "{refused:?}");
    assert!(matches!(unread, Err(Unkept::Unreadable(_))), "{unread:?}");
    assert!(!format!("{:?}", made_for("jellyfin")).contains("pkcs12:"));
}
