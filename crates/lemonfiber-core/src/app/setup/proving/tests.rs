use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::Mutex;

use async_trait::async_trait;

use super::{assessed, proven};
use crate::model::StorageFinding;
use crate::ports::filesystem::{Fault, FsKind, Identity, StorageFacts};
use crate::validate::{Credential, Validation, Validator};
use crate::wizard::{Answer, Indexer, Provider};
use lemonfiber_fixtures::probing::Probing;

/// A filesystem on which no link ever takes, reporting `kind`.
fn cannot_link(kind: FsKind) -> Probing {
    let linking = Probing::linking();
    Probing {
        failing_links: AtomicUsize::new(usize::MAX),
        facts: StorageFacts {
            kind,
            ..linking.facts
        },
        ..linking
    }
}

/// A validator that proves everything and keeps what it was asked to prove.
#[derive(Default)]
struct Keeping(Mutex<Vec<Credential>>);

#[async_trait]
impl Validator for Keeping {
    async fn validate(&self, credential: &Credential) -> Validation {
        if let Ok(mut seen) = self.0.lock() {
            seen.push(credential.clone());
        }
        Validation::Valid {
            observed: "answered".to_owned(),
        }
    }
}

#[tokio::test]
async fn a_location_that_links_is_found_to_link_on_itself() {
    let found = assessed(&Probing::linking(), Path::new("/media")).await;

    assert_eq!(
        found,
        StorageFinding::Links {
            inferred_from: None
        }
    );
}

#[tokio::test]
async fn a_location_not_there_yet_is_tested_on_its_parent_and_says_so() {
    let filesystem = Probing {
        missing_leaves: AtomicUsize::new(1),
        ..Probing::linking()
    };

    let found = assessed(&filesystem, Path::new("/srv/media")).await;

    assert_eq!(
        found,
        StorageFinding::Links {
            inferred_from: Some(PathBuf::from("/srv"))
        }
    );
}

#[tokio::test]
async fn a_location_that_cannot_link_names_its_filesystem_reason() {
    let found = assessed(&cannot_link(FsKind::ExFat), Path::new("/media")).await;

    assert!(matches!(
        found,
        StorageFinding::CopyOnly { limitation: Some(reason) } if reason.contains("exFAT")
    ));
}

#[tokio::test]
async fn a_copy_only_location_names_no_reason_where_its_type_explains_nothing() {
    let found = assessed(
        &cannot_link(FsKind::Linking("ext4".to_owned())),
        Path::new("/media"),
    )
    .await;

    assert_eq!(found, StorageFinding::CopyOnly { limitation: None });
}

#[tokio::test]
async fn a_location_nothing_of_which_can_be_reached_is_untested() {
    let filesystem = Probing {
        resolves: Err(Fault::new("no such file or directory")),
        ..Probing::linking()
    };

    let found = assessed(&filesystem, Path::new("/nowhere/media")).await;

    assert!(matches!(found, StorageFinding::Untested { .. }));
}

#[tokio::test]
async fn a_location_that_cannot_be_written_is_untested_in_the_platforms_words() {
    let filesystem = Probing {
        writes: Err(Fault::new("permission denied")),
        ..Probing::linking()
    };

    let found = assessed(&filesystem, Path::new("/media")).await;

    assert!(matches!(
        found,
        StorageFinding::Untested { reason } if reason.contains("permission denied")
    ));
}

#[tokio::test]
async fn a_link_that_cannot_be_confirmed_is_untested() {
    let filesystem = Probing {
        confirmed: Ok(Identity { file: 8, links: 2 }),
        ..Probing::linking()
    };

    let found = assessed(&filesystem, Path::new("/media")).await;

    assert!(matches!(found, StorageFinding::Untested { .. }));
}

#[tokio::test]
async fn a_data_location_answer_comes_back_with_what_its_test_found() {
    let (answer, proof) = proven(
        &Keeping::default(),
        &Probing::linking(),
        Answer::DataLocation(PathBuf::from("/media")),
    )
    .await;

    assert_eq!(answer, Answer::DataLocation(PathBuf::from("/media")));
    assert!(matches!(proof.storage, Some(StorageFinding::Links { .. })));
    assert!(proof.credential.is_none());
}

#[tokio::test]
async fn a_pasted_indexer_is_trimmed_before_it_is_tested_and_kept() {
    let validator = Keeping::default();

    let (answer, proof) = proven(
        &validator,
        &Probing::linking(),
        Answer::Credentials(Some(Indexer {
            url: " http://indexer.test/api\n".to_owned(),
            key: "\tthe-key ".to_owned(),
            validated: false,
        })),
    )
    .await;

    assert_eq!(
        answer,
        Answer::Credentials(Some(Indexer {
            url: "http://indexer.test/api".to_owned(),
            key: "the-key".to_owned(),
            validated: true,
        }))
    );
    assert!(matches!(
        validator.0.lock().map(|seen| seen.clone()).unwrap_or_default().as_slice(),
        [Credential::Indexer { url, key }] if url == "http://indexer.test/api" && key == "the-key"
    ));
    assert!(proof.storage.is_none());
}

#[tokio::test]
async fn a_pasted_provider_is_trimmed_before_it_is_tested_and_kept() {
    let (answer, proof) = proven(
        &Keeping::default(),
        &Probing::linking(),
        Answer::Provider(Some(Provider {
            host: " news.test ".to_owned(),
            port: 563,
            user: "someone\n".to_owned(),
            pass: " the-pass".to_owned(),
            tls: true,
            validated: false,
        })),
    )
    .await;

    assert!(matches!(
        answer,
        Answer::Provider(Some(Provider { host, user, pass, validated: true, .. }))
            if host == "news.test" && user == "someone" && pass == "the-pass"
    ));
    assert!(matches!(proof.credential, Some(Validation::Valid { .. })));
}

#[tokio::test]
async fn an_answer_that_asks_no_service_is_kept_as_given_with_nothing_proven() {
    let validator = Keeping::default();

    let (answer, proof) = proven(&validator, &Probing::linking(), Answer::Household(true)).await;

    assert_eq!(answer, Answer::Household(true));
    assert!(proof.credential.is_none() && proof.storage.is_none());
    assert!(validator.0.lock().is_ok_and(|seen| seen.is_empty()));
}
