//! Proving, keeping or skipping an indexer key and a Usenet provider.

use super::*;

/// A key pasted with whitespace around it is trimmed before it is proven and
/// before it is kept, so the value tested is the value stored.
///
/// Trimming at only one of the two would be worse than trimming at neither: the
/// operator would watch the key prove itself and then find the stored one does not
/// work, which is the silent failure this whole feature exists to prevent.
#[tokio::test]
async fn a_pasted_key_is_trimmed_before_it_is_proven_and_before_it_is_kept() {
    let dir = scratch("cred-pasted");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = Scripted {
        credential: Some((
            "  http://indexer.test/api\n".to_owned(),
            "\tthe-key \n".to_owned(),
        )),
        on_failure: CredentialChoice::Skip,
        ..Scripted::workable(dir.join("data-root"))
    };
    let validator = Proving::giving(vec![Validation::Valid {
        observed: "answered a search — 12 result(s) offered".to_owned(),
    }]);

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &validator,
        &applying(&paths, "t"),
    )
    .await;
    assert!(matches!(outcome, Ok(Outcome::Applied)));

    // What the service was actually asked about — not merely that a call was made.
    let asked = validator.asked();
    assert_eq!(
        asked.as_slice(),
        [Credential::Indexer {
            url: "http://indexer.test/api".to_owned(),
            key: "the-key".to_owned(),
        }]
    );

    // ...and the same string is what was written down.
    let file = store::read(&paths.env_file()).unwrap_or_default();
    assert_eq!(file.get("INDEXER_APIKEY"), Some("the-key"));
    assert_eq!(file.get("INDEXER_URL"), Some("http://indexer.test/api"));
}

#[tokio::test]
async fn a_credential_proven_is_kept_and_recorded_as_validated() {
    let dir = scratch("cred-valid");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = entering(&dir, CredentialChoice::Skip);
    let validator = Proving::giving(vec![Validation::Valid {
        observed: "answered a search — 12 result(s) offered".to_owned(),
    }]);

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &validator,
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    let file = store::read(&paths.env_file()).unwrap_or_default();
    assert_eq!(file.get("INDEXER_APIKEY"), Some("the-key"));
    assert_eq!(file.get("INDEXER_VALIDATED"), Some("on"));
    // The operator was shown what the test observed, not a bare pass.
    assert!(prompt
        .proven
        .borrow()
        .iter()
        .any(|observed| observed.contains("12 result")));
}

#[tokio::test]
async fn a_credential_that_fails_then_passes_on_retry_is_kept() {
    let dir = scratch("cred-retry");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = entering(&dir, CredentialChoice::Retry);
    // The first test refuses the key, the second — after the operator re-enters
    // it — proves it.
    let validator = Proving::giving(vec![
        Validation::Rejected {
            detail: "the indexer refused the key".to_owned(),
        },
        Validation::Valid {
            observed: "answered a search".to_owned(),
        },
    ]);

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &validator,
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    assert_eq!(prompt.failures.borrow().len(), 1, "one refusal was shown");
    let file = store::read(&paths.env_file()).unwrap_or_default();
    assert_eq!(file.get("INDEXER_VALIDATED"), Some("on"));
}

#[tokio::test]
async fn a_credential_kept_unverified_records_that_it_was_not_proven() {
    let dir = scratch("cred-proceed");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = entering(&dir, CredentialChoice::Proceed);
    let validator = Proving::giving(vec![Validation::Unreachable {
        detail: "nothing answered".to_owned(),
    }]);

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &validator,
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    let file = store::read(&paths.env_file()).unwrap_or_default();
    // Kept, so it is not lost — but recorded as unverified so a later diagnosis
    // can point at it rather than trusting it.
    assert_eq!(file.get("INDEXER_APIKEY"), Some("the-key"));
    assert_eq!(file.get("INDEXER_VALIDATED"), Some("off"));
}

#[tokio::test]
async fn a_credential_skipped_leaves_the_indexer_unset() {
    let dir = scratch("cred-skip");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = entering(&dir, CredentialChoice::Skip);
    let validator = Proving::giving(vec![Validation::Rejected {
        detail: "refused".to_owned(),
    }]);

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &validator,
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    let file = store::read(&paths.env_file()).unwrap_or_default();
    assert_eq!(file.get("INDEXER_URL"), None, "nothing was written for it");
}

#[tokio::test]
async fn no_indexer_is_a_supported_end_and_persists_nothing_before_a_test() {
    let dir = scratch("cred-none");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    // The workable prompt enters no credential — a supported path, and one that
    // never reaches the validator, so nothing about a credential is persisted
    // without a test having run.
    let prompt = Scripted::workable(dir.join("data-root"));

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &proving(),
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    let file = store::read(&paths.env_file()).unwrap_or_default();
    assert_eq!(file.get("INDEXER_URL"), None);
}

/// A prompt that enters the given Usenet provider and answers a failed login
/// the given way — everything else the workable defaults.
fn entering_provider(dir: &Path, on_failure: CredentialChoice) -> Scripted {
    Scripted {
        provider: Some(ProviderEntry {
            host: "news.provider.test".to_owned(),
            port: 563,
            user: "person".to_owned(),
            pass: "secret".to_owned(),
            tls: true,
        }),
        on_failure,
        ..Scripted::workable(dir.join("data-root"))
    }
}

#[tokio::test]
async fn a_usenet_provider_proven_is_kept_and_recorded_as_validated() {
    let dir = scratch("provider-valid");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = entering_provider(&dir, CredentialChoice::Skip);
    let validator = Proving::giving(vec![Validation::Valid {
        observed: "the provider accepted the login".to_owned(),
    }]);

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &validator,
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    let file = store::read(&paths.env_file()).unwrap_or_default();
    assert_eq!(file.get("USENET_HOST"), Some("news.provider.test"));
    assert_eq!(file.get("USENET_USER"), Some("person"));
    assert_eq!(file.get("USENET_VALIDATED"), Some("on"));
    assert!(prompt
        .proven
        .borrow()
        .iter()
        .any(|observed| observed.contains("accepted the login")));
}

#[tokio::test]
async fn a_provider_that_fails_then_passes_on_retry_is_kept() {
    let dir = scratch("provider-retry");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = entering_provider(&dir, CredentialChoice::Retry);
    let validator = Proving::giving(vec![
        Validation::Rejected {
            detail: "the provider refused the username or password".to_owned(),
        },
        Validation::Valid {
            observed: "the provider accepted the login".to_owned(),
        },
    ]);

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &validator,
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    assert_eq!(prompt.failures.borrow().len(), 1);
    let file = store::read(&paths.env_file()).unwrap_or_default();
    assert_eq!(file.get("USENET_VALIDATED"), Some("on"));
}

#[tokio::test]
async fn a_provider_kept_unverified_records_that_it_was_not_proven() {
    let dir = scratch("provider-proceed");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = entering_provider(&dir, CredentialChoice::Proceed);
    let validator = Proving::giving(vec![Validation::Unreachable {
        detail: "nothing answered".to_owned(),
    }]);

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &validator,
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    let file = store::read(&paths.env_file()).unwrap_or_default();
    assert_eq!(file.get("USENET_HOST"), Some("news.provider.test"));
    assert_eq!(file.get("USENET_VALIDATED"), Some("off"));
}

#[tokio::test]
async fn a_provider_skipped_leaves_usenet_unset() {
    let dir = scratch("provider-skip");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    let prompt = entering_provider(&dir, CredentialChoice::Skip);
    let validator = Proving::giving(vec![Validation::Rejected {
        detail: "refused".to_owned(),
    }]);

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &validator,
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    let file = store::read(&paths.env_file()).unwrap_or_default();
    assert_eq!(file.get("USENET_HOST"), None);
}

#[tokio::test]
async fn a_library_only_run_is_never_asked_for_a_credential() {
    let dir = scratch("cred-library-only");
    let paths = layout(&dir);
    let mut wizard = Wizard::new(Environment::LinuxNative);
    // Neither protocol chosen, so there is no download service to hold a
    // credential — the step does not apply and is passed over, even though the
    // prompt would have offered one.
    let prompt = Scripted {
        protocols: Protocols::none(),
        credential: Some(("http://indexer.test/api".to_owned(), "k".to_owned())),
        ..Scripted::workable(dir.join("data-root"))
    };
    // A validator that would fail if it were ever asked, proving it is not.
    let validator = Proving::giving(vec![Validation::Rejected {
        detail: "should never be reached".to_owned(),
    }]);

    let outcome = run(
        &mut wizard,
        &prompt,
        &ProbeFs::links(),
        &validator,
        &applying(&paths, "t"),
    )
    .await;

    assert!(matches!(outcome, Ok(Outcome::Applied)));
    assert!(
        prompt.failures.borrow().is_empty(),
        "no credential was tested"
    );
    let file = store::read(&paths.env_file()).unwrap_or_default();
    assert_eq!(file.get("INDEXER_URL"), None);
}
