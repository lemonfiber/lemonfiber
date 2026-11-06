use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use lemonfiber_adapters::Disk;
use lemonfiber_fixtures::scratch::Scratch;

use super::{kept, kept_or_made, made, withdrawn, written, Beneath, AUTHORITY, CERTIFICATE, KEY};

/// The gate's and the request service's configuration directories beneath `scratch`.
fn directories(scratch: &Scratch) -> (PathBuf, PathBuf) {
    let gate = scratch.path().join("request-gate");
    let requests = scratch.path().join("seerr");
    for one in [&gate, &requests] {
        let _ = std::fs::create_dir_all(one);
    }
    (gate, requests)
}

/// What `openssl verify` says of the certificate beneath `gate` against the authority
/// beneath `requests`, for `host`.
fn verified(gate: &Path, requests: &Path, host: &str) -> bool {
    Command::new("openssl")
        .args(["verify", "-verify_hostname", host, "-CAfile"])
        .arg(requests.join(AUTHORITY))
        .arg(gate.join(CERTIFICATE))
        .output()
        .is_ok_and(|ran| ran.status.success())
}

/// The permission bits of `at`.
fn mode(at: &Path) -> u32 {
    std::fs::metadata(at).map_or(0, |held| held.permissions().mode() & 0o777)
}

#[test]
fn the_authority_signs_one_certificate_for_plex_tv_alone_and_the_gate_alone_reads_it() {
    let scratch = Scratch::new("fronted-made");
    let (gate, requests) = directories(&scratch);
    let at = Beneath {
        gate: &gate,
        requests: &requests,
    };

    assert_eq!(kept_or_made(&Disk, at), Ok(true));

    assert_eq!(kept(&Disk, at), Ok(true));
    assert!(verified(&gate, &requests, "plex.tv"));
    assert!(verified(&gate, &requests, "discover.provider.plex.tv"));
    assert!(!verified(&gate, &requests, "example.com"));
    assert_eq!(mode(&gate.join(KEY)), 0o600);
    assert_eq!(mode(&gate.join(CERTIFICATE)), 0o600);
}

#[test]
fn what_is_kept_is_kept_and_anything_else_is_made_anew() {
    let scratch = Scratch::new("fronted-kept");
    let (gate, requests) = directories(&scratch);
    let at = Beneath {
        gate: &gate,
        requests: &requests,
    };
    assert_eq!(kept(&Disk, at), Ok(false));
    assert_eq!(kept_or_made(&Disk, at), Ok(true));
    let first = std::fs::read_to_string(requests.join(AUTHORITY)).unwrap_or_default();

    assert_eq!(kept_or_made(&Disk, at), Ok(false));
    assert_eq!(
        std::fs::read_to_string(requests.join(AUTHORITY)).unwrap_or_default(),
        first
    );

    let other = made().map(|one| one.authority);
    let _ = std::fs::write(requests.join(AUTHORITY), other.unwrap_or_default());
    assert_eq!(kept(&Disk, at), Ok(false));
    assert_eq!(kept_or_made(&Disk, at), Ok(true));
    assert!(verified(&gate, &requests, "plex.tv"));
}

#[test]
fn a_key_that_is_none_is_not_kept() {
    let scratch = Scratch::new("fronted-keyless");
    let (gate, requests) = directories(&scratch);
    let at = Beneath {
        gate: &gate,
        requests: &requests,
    };
    let fronted = made();
    assert!(fronted
        .as_ref()
        .is_ok_and(|one| written(&Disk, at, one).is_ok()));

    let _ = std::fs::write(gate.join(KEY), "not a key");

    assert_eq!(kept(&Disk, at), Ok(false));
}

#[test]
fn leaving_takes_every_file_away_and_finding_none_is_no_fault() {
    let scratch = Scratch::new("fronted-withdrawn");
    let (gate, requests) = directories(&scratch);
    let at = Beneath {
        gate: &gate,
        requests: &requests,
    };
    assert_eq!(kept_or_made(&Disk, at), Ok(true));

    assert_eq!(withdrawn(&Disk, at), Ok(()));

    for gone in [
        requests.join(AUTHORITY),
        gate.join(CERTIFICATE),
        gate.join(KEY),
    ] {
        assert!(!gone.exists(), "{}", gone.display());
    }
    assert_eq!(withdrawn(&Disk, at), Ok(()));
}

#[test]
fn what_is_printed_of_one_is_that_it_is_one() {
    let printed = made().map(|one| format!("{one:?}")).unwrap_or_default();

    assert_eq!(printed, "Fronted { .. }");
}

#[test]
fn an_authority_that_is_none_or_a_certificate_naming_more_is_not_kept() {
    let scratch = Scratch::new("fronted-unkept");
    let (gate, requests) = directories(&scratch);
    let at = Beneath {
        gate: &gate,
        requests: &requests,
    };
    assert_eq!(kept_or_made(&Disk, at), Ok(true));
    let authority = requests.join(AUTHORITY);
    let held = std::fs::read_to_string(&authority).unwrap_or_default();

    for unread in [
        "not a certificate".to_owned(),
        "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n".to_owned(),
    ] {
        let _ = std::fs::write(&authority, unread);
        assert_eq!(kept(&Disk, at), Ok(false));
    }
    let _ = std::fs::write(&authority, held);
    let mut wider = rcgen::CertificateParams::new(vec!["plex.tv".to_owned()]).ok();
    if let Some(params) = wider.as_mut() {
        params
            .subject_alt_names
            .push(rcgen::SanType::IpAddress(std::net::IpAddr::from([
                127, 0, 0, 1,
            ])));
    }
    let signed = wider
        .zip(rcgen::KeyPair::generate().ok())
        .and_then(|(params, key)| params.self_signed(&key).ok())
        .map(|one| super::pem(super::CERTIFIED, one.der()))
        .unwrap_or_default();
    let _ = std::fs::write(gate.join(CERTIFICATE), signed);
    assert_eq!(kept(&Disk, at), Ok(false));
}

#[test]
fn a_file_reached_through_a_link_is_neither_read_written_nor_taken_away() {
    let scratch = Scratch::new("fronted-linked");
    let (gate, requests) = directories(&scratch);
    let elsewhere = scratch.path().join("elsewhere");
    let _ = std::fs::create_dir_all(&elsewhere);
    assert!(std::os::unix::fs::symlink(&elsewhere, requests.join("plex-tv")).is_ok());
    let at = Beneath {
        gate: &gate,
        requests: &requests,
    };

    assert!(matches!(kept(&Disk, at), Err(super::Unkept::Unreadable(_))));
    assert!(matches!(
        made().map(|one| written(&Disk, at, &one)),
        Ok(Err(super::Unkept::Unmade(_)))
    ));
    assert!(matches!(
        withdrawn(&Disk, at),
        Err(super::Unkept::Unmade(_))
    ));
    assert!(std::fs::read_dir(&elsewhere).is_ok_and(|mut entries| entries.next().is_none()));
}
