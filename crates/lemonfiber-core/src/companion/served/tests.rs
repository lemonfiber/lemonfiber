use lemonfiber_fixtures::scratch::Scratch;

use super::{last, record, Served, FILE};

/// What a run wrote is what pairing reads back.
#[test]
fn how_the_surface_was_served_is_read_back_as_written() {
    let at = Scratch::new("companion-served");
    assert_eq!(last(&at), None);
    let served = Served {
        port: 8443,
        encrypted: true,
        network: true,
    };
    assert_eq!(record(&at, served), Ok(()));
    assert_eq!(last(&at), Some(served));
}

/// Something that is not this record reads as no record, rather than as a port.
#[test]
fn a_record_that_is_not_one_reads_as_none() {
    let at = Scratch::new("companion-served-unreadable");
    let _ = std::fs::write(at.join(FILE), r#"{"port": 8443}"#);
    assert_eq!(last(&at), None);
}

/// A record that cannot be written says why.
#[test]
fn a_record_that_cannot_be_written_says_why() {
    let at = Scratch::new("companion-served-unwritable");
    let blocked = at.join("blocked");
    let _ = std::fs::write(&blocked, "a file where the directory would go");
    let written = record(
        &blocked,
        Served {
            port: 1,
            encrypted: false,
            network: false,
        },
    );
    assert!(
        written
            .as_ref()
            .is_err_and(|why| !why.is_empty() && !why.contains(&*at.to_string_lossy())),
        "{written:?}"
    );
}

/// A context keeping what pairing needs in `directory`.
fn keeping(directory: &std::path::Path) -> crate::app::Ctx {
    crate::test_support::a_context()
        .settings(crate::config::Settings {
            companion: Some(directory.to_path_buf()),
            ..crate::config::Settings::default()
        })
        .build()
}

/// What a dispatch answered with, where it answered how the surface is served.
async fn answered(
    command: crate::app::Command,
    ctx: &crate::app::Ctx,
) -> Result<super::ServedReport, Box<crate::error::Problem>> {
    match crate::app::dispatch(command, ctx).await? {
        crate::app::Outcome::Served(report) => Ok(report),
        _ => Ok(super::ServedReport::default()),
    }
}

/// Every file in `directory`, with what it holds, in name order.
fn held(directory: &std::path::Path) -> Vec<(std::ffi::OsString, Vec<u8>)> {
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| {
            (
                entry.file_name(),
                std::fs::read(entry.path()).unwrap_or_default(),
            )
        })
        .collect();
    files.sort();
    files
}

/// Asked of a run that settled nothing, how the surface is served is refused rather than
/// settled.
#[tokio::test]
async fn how_the_surface_is_served_is_refused_where_nothing_was_settled() {
    let at = Scratch::new("served-unsettled");
    let ctx = keeping(&at);
    let read = answered(crate::app::Command::Serving, &ctx).await;
    assert_eq!(
        read.err().map(|problem| problem.code),
        Some(crate::error::codes::serve::NOT_SERVING)
    );
    assert!(held(&at).is_empty(), "asking made or wrote something");
}

/// The read answers what settling answered, without the key, and makes, replaces and
/// writes nothing: what settling kept and wrote is taken away, and asking puts none of it
/// back.
#[tokio::test]
async fn how_the_surface_is_served_is_read_from_what_was_settled_and_never_settled_again() {
    let at = Scratch::new("served-settled");
    let ctx = keeping(&at);
    let settled = answered(
        crate::app::Command::Served(Served {
            port: 8443,
            encrypted: true,
            network: false,
        }),
        &ctx,
    )
    .await;
    let Ok(settled) = settled else {
        unreachable!("a scratch directory keeps a certificate")
    };
    assert!(settled.presented.is_some());
    for file in [FILE, "certificate.pem", "key.pem"] {
        assert!(std::fs::remove_file(at.join(file)).is_ok(), "{file}");
    }

    let read = answered(crate::app::Command::Serving, &ctx).await;

    assert_eq!(
        read,
        Ok(super::ServedReport {
            presented: None,
            ..settled.clone()
        })
    );
    assert!(held(&at).is_empty(), "asking made or wrote something");
    let certificate = settled.certificate.as_ref();
    assert_eq!(
        certificate.map(|described| described.fingerprint.as_str()),
        settled
            .presented
            .as_ref()
            .map(|kept| kept.fingerprint.as_str())
    );
    assert_eq!(
        (settled.port, settled.encrypted, settled.network),
        (8443, true, false)
    );
}

/// What a run serves plain carries no certificate.
#[tokio::test]
async fn a_run_served_plain_carries_no_certificate() {
    let at = Scratch::new("served-plain");
    let ctx = keeping(&at);
    let settled = answered(
        crate::app::Command::Served(Served {
            port: 7171,
            encrypted: false,
            network: false,
        }),
        &ctx,
    )
    .await;
    assert!(settled.is_ok_and(|report| report.certificate.is_none() && report.port == 7171));
    let read = answered(crate::app::Command::Serving, &ctx).await;
    assert!(read.is_ok_and(|report| report.certificate.is_none() && !report.encrypted));
}

/// Nothing secret is in the answer: not the key or the certificate in any encoding, and
/// no path.
#[tokio::test]
async fn how_the_surface_is_served_carries_no_key_no_encoding_and_no_path() {
    use base64::Engine as _;
    let at = Scratch::new("served-secret");
    let ctx = keeping(&at);
    let settled = answered(
        crate::app::Command::Served(Served {
            port: 8443,
            encrypted: true,
            network: true,
        }),
        &ctx,
    )
    .await;
    let Some((certificate, key)) = settled
        .ok()
        .and_then(|report| report.presented)
        .map(|kept| kept.presented())
    else {
        unreachable!("an encrypted run presents a certificate")
    };
    let read = answered(crate::app::Command::Serving, &ctx).await;
    let Some(said) = read
        .ok()
        .and_then(|report| crate::app::Outcome::Served(report).envelope().to_json())
    else {
        unreachable!("what was settled is read and rendered")
    };
    let encoded = |der: &[u8]| base64::engine::general_purpose::STANDARD.encode(der);
    let directory = at.to_string_lossy().into_owned();
    for (what, secret) in [
        ("the key in base64", encoded(&key)),
        ("the key in hex", crate::secret::render(&key)),
        ("the certificate in base64", encoded(&certificate)),
        (
            "the certificate in hex",
            crate::secret::render(&certificate),
        ),
        ("a PEM marker", "-----".to_owned()),
        ("a PEM label", "PRIVATE KEY".to_owned()),
        ("the directory", directory),
        ("the record's name", FILE.to_owned()),
        ("the certificate's file", "certificate.pem".to_owned()),
        ("the key's file", "key.pem".to_owned()),
    ] {
        let start: String = secret.chars().take(24).collect();
        assert!(!said.contains(&start), "the answer carries {what}: {said}");
    }
    assert!(said.contains("\"certificate\":{"), "{said}");
}
