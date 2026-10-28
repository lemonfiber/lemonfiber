use lemonfiber_fixtures::scratch::Scratch;

use super::{
    der, fingerprint, kept, kept_or_made, pem, replaced, unmade, Unkept, CERTIFICATE, KEY,
};

/// What a phone pins is SHA-256 over the DER encoding, in lower-case hex, and nothing
/// else of the same length.
#[test]
fn a_fingerprint_is_the_digest_of_the_whole_certificate_in_lower_case_hex() {
    assert_eq!(
        fingerprint(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

/// Made once and presented unchanged after, so a phone paired once goes on recognising
/// this machine; and what is presented is the pair a TLS server can hold.
#[test]
fn a_certificate_is_made_once_and_kept() {
    let at = Scratch::new("companion-certificate-kept");
    assert_eq!(
        kept(&lemonfiber_adapters::Disk, &at),
        Ok(None),
        "nothing is made by looking"
    );

    let made = kept_or_made(&lemonfiber_adapters::Disk, &at);
    let again = kept_or_made(&lemonfiber_adapters::Disk, &at);
    assert!(made.is_ok(), "{made:?}");
    assert_eq!(made, again, "the second run presents what the first made");
    assert!(
        made.as_ref()
            .is_ok_and(|held| held.fingerprint.len() == 64 && !held.presented().0.is_empty()),
        "{made:?}"
    );
    let described = format!("{made:?}");
    assert!(
        !described.contains("PRIVATE KEY"),
        "the key is never printed: {described}"
    );
}

/// Replacing it is a different certificate, which is exactly why it is said first.
#[test]
fn a_replaced_certificate_is_a_different_one() {
    let at = Scratch::new("companion-certificate-replaced");
    let first = kept_or_made(&lemonfiber_adapters::Disk, &at).map(|held| held.fingerprint);
    let second = replaced(&lemonfiber_adapters::Disk, &at).map(|held| held.fingerprint);
    assert!(first.is_ok() && second.is_ok());
    assert_ne!(first, second);
    assert_eq!(
        kept(&lemonfiber_adapters::Disk, &at).map(|held| held.map(|held| held.fingerprint)),
        second.map(Some),
        "and it is the one kept from then on"
    );
}

/// Half a pair, or a file that is not what it should be, is refused rather than made
/// again over: a new certificate is every paired phone refusing this machine.
#[test]
fn what_cannot_be_read_is_refused_rather_than_made_again() {
    let at = Scratch::new("companion-certificate-unreadable");
    let _ = kept_or_made(&lemonfiber_adapters::Disk, &at);
    let _ = std::fs::remove_file(at.join(CERTIFICATE));
    assert!(
        matches!(kept_or_made(&lemonfiber_adapters::Disk, &at), Err(Unkept::Unreadable(why)) if why.contains("only one"))
    );

    let _ = std::fs::write(at.join(CERTIFICATE), "not a certificate");
    assert!(
        matches!(kept(&lemonfiber_adapters::Disk, &at), Err(Unkept::Unreadable(why)) if why.contains("not a certificate"))
    );

    let _ = replaced(&lemonfiber_adapters::Disk, &at);
    let _ = std::fs::write(at.join(KEY), "not a key");
    assert!(
        matches!(kept(&lemonfiber_adapters::Disk, &at), Err(Unkept::Unreadable(why)) if why.contains("not a private key"))
    );
}

/// A directory that cannot be written to makes nothing, and says which file.
#[test]
fn a_certificate_that_cannot_be_written_down_is_not_made() {
    let at = Scratch::new("companion-certificate-unwritable");
    let blocked = at.join("blocked");
    let _ = std::fs::write(&blocked, "a file where the directory would go");
    assert!(
        matches!(replaced(&lemonfiber_adapters::Disk, &blocked), Err(Unkept::Unmade(why)) if why.contains(KEY)),
        "the key is written first"
    );
}

/// A block that opens and never closes is not a certificate, whatever follows it.
#[test]
fn a_block_that_is_never_closed_holds_nothing() {
    assert_eq!(
        der("-----BEGIN CERTIFICATE-----\nAAAA\n", "CERTIFICATE"),
        None
    );
    assert_eq!(
        der(
            "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n",
            "CERTIFICATE"
        ),
        Some(vec![0, 0, 0])
    );
}

/// What stopped a certificate being made is said, not swallowed.
#[test]
fn a_certificate_that_could_not_be_made_says_why() {
    assert!(matches!(
        unmade(&rcgen::Error::RingUnspecified),
        Unkept::Unmade(why) if why.starts_with("a certificate could not be made: ")
    ));
}

/// What is written is what is read back, broken into the lines PEM readers expect.
#[test]
fn a_block_written_reads_back_as_what_it_holds() {
    let held: Vec<u8> = (0..=255).collect();
    let written = pem("CERTIFICATE", &held);
    assert!(written.starts_with("-----BEGIN CERTIFICATE-----\n"));
    assert!(written.ends_with("\n-----END CERTIFICATE-----\n"));
    assert!(written.lines().all(|line| line.len() <= 64), "{written}");
    assert_eq!(der(&written, "CERTIFICATE"), Some(held));
}

/// A container that can write the directory the door's certificate is kept in cannot
/// point the core's read of it anywhere else.
#[cfg(unix)]
#[test]
fn a_link_planted_where_the_certificate_or_its_key_is_kept_is_never_followed() {
    use std::os::unix::fs::symlink;
    let scratch = Scratch::new("certificate-linked");
    let at = scratch.join("door");
    let _ = std::fs::create_dir_all(&at);
    let elsewhere = scratch.join("elsewhere");
    let disk = &lemonfiber_adapters::Disk;
    let _ = kept_or_made(disk, &elsewhere);

    for name in [KEY, CERTIFICATE] {
        let _ = std::fs::remove_file(at.join(KEY));
        let _ = std::fs::remove_file(at.join(CERTIFICATE));
        let _ = kept_or_made(disk, &at);
        let _ = std::fs::remove_file(at.join(name));
        let _ = symlink(elsewhere.join(name), at.join(name));
        assert!(
            matches!(kept(disk, &at), Err(Unkept::Unreadable(why)) if why.contains(name)),
            "{name}"
        );
    }
    assert!(replaced(disk, &at).is_ok());
    assert!(std::fs::symlink_metadata(at.join(KEY)).is_ok_and(|meta| meta.is_file()));
    assert!(kept(disk, &at).is_ok_and(|kept| kept.is_some()));
}
