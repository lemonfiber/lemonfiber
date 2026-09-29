use super::signing::Signing;
use super::{carried, signer, verified, Entry, Refused};

/// An index as the catalogue's release writes one: sorted keys, two-space indent, one
/// trailing newline.
const INDEX: &str = r#"{
  "plugins": [
    {
      "id": "komga",
      "manifest": "sha256:3e5a6b0d3f2c3a0a64f3cb6bd4c2f69f0f7f06a6b7c1a0f3d7ce5f1d3b1b0f4a",
      "origin": "https://github.com/lemonfiber/plugin-komga",
      "revision": "8fa05ba718f70624f2c122f8c0371d47e6c90d0e"
    }
  ],
  "schema": 1
}
"#;

/// Why an index was refused, in one word and its sentence, or `read` where it was not.
fn came_to(index: &str, signature: &str, signing: &Signing) -> (String, String) {
    match verified(index, signature, signing.key().as_ref()) {
        Ok(_) => ("read".to_owned(), String::new()),
        Err(Refused::Unverified(why)) => ("unverified".to_owned(), why),
        Err(Refused::Unreadable(why)) => ("unreadable".to_owned(), why),
    }
}

/// An index whose signature the carried key verifies is read, and a name resolves
/// through it to the origin and commit it registered.
#[test]
fn a_signed_index_resolves_a_name_to_what_was_reviewed() {
    let read = Signing::new()
        .and_then(|signing| verified(INDEX, &signing.signed(INDEX), signing.key().as_ref()).ok());

    let entry = read.as_ref().and_then(|index| index.entry("komga"));
    assert_eq!(
        entry.map(|one| (one.origin.as_str(), one.revision.as_str())),
        Some((
            "https://github.com/lemonfiber/plugin-komga",
            "8fa05ba718f70624f2c122f8c0371d47e6c90d0e"
        ))
    );
    assert!(read
        .as_ref()
        .is_some_and(|index| index.entry("kuma").is_none()));
}

/// A signature over other bytes, one made by another key, and one that is not a
/// signature at all are each refused as unverified, and nothing is read.
#[test]
fn a_signature_that_does_not_hold_is_refused() {
    let pairs = Signing::new().zip(Signing::new());
    assert!(pairs.is_some(), "no key pair could be made");
    let altered = INDEX.replace("8fa05ba7", "9fa05ba7");

    if let Some((ours, theirs)) = pairs {
        for (index, signature) in [
            (altered.as_str(), ours.signed(INDEX)),
            (INDEX, theirs.signed(INDEX)),
            (INDEX, "not base64 at all".to_owned()),
            (INDEX, String::new()),
        ] {
            let (word, why) = came_to(index, &signature, &ours);
            assert_eq!(word, "unverified", "{signature:?} was {word}: {why}");
        }
        let (_, why) = came_to(INDEX, &theirs.signed(INDEX), &ours);
        assert!(
            why.contains("does not verify against the catalogue's test key (sha256:"),
            "{why}"
        );
    }
}

/// A build carrying no key verifies nothing, so every index is refused however it
/// is signed.
#[test]
fn a_build_with_no_key_refuses_every_index() {
    let refused = Signing::new().map(|signing| verified(INDEX, &signing.signed(INDEX), None));

    assert!(
        matches!(refused, Some(Err(Refused::Unverified(ref why))) if why.contains("carries no key")),
        "{refused:?}"
    );
    assert!(carried().is_none(), "this build carries a catalogue key");
}

/// A signed index this build cannot read, or of a shape it does not know, is refused
/// as unreadable rather than read in part.
#[test]
fn a_signed_index_of_another_shape_is_unreadable() {
    let signing = Signing::new();
    assert!(signing.is_some(), "no key pair could be made");
    let later = INDEX.replace("\"schema\": 1", "\"schema\": 2");

    if let Some(signing) = signing {
        for index in [later.as_str(), "[]\n", "{\"schema\": 1}\n"] {
            let (word, why) = came_to(index, &signing.signed(index), &signing);
            assert_eq!(word, "unreadable", "{index} was {word}: {why}");
        }
        let (_, why) = came_to(&later, &signing.signed(&later), &signing);
        assert!(
            why.contains("shape 2 and this build reads shape 1"),
            "{why}"
        );
    }
}

/// An entry holds a manifest whose bytes hash to the digest it names, and no other.
#[test]
fn an_entry_holds_only_the_manifest_it_names() {
    let manifest = b"schema_version = 1\n";
    let entry = |digest: &str| Entry {
        id: "komga".to_owned(),
        origin: "https://example.org/plugin-komga".to_owned(),
        revision: "8fa05ba718f70624f2c122f8c0371d47e6c90d0e".to_owned(),
        manifest: digest.to_owned(),
    };
    let digest = format!(
        "sha256:{}",
        crate::secret::render(ring::digest::digest(&ring::digest::SHA256, manifest).as_ref())
    );

    assert!(entry(&digest).holds(manifest));
    assert!(!entry(&digest).holds(b"schema_version = 2\n"));
    assert!(!entry(&digest.replace("sha256:", "sha512:")).holds(manifest));
}

/// Only a whole commit is a revision an index may pin a plugin to.
#[test]
fn only_a_whole_commit_is_pinned() {
    let entry = |revision: &str| Entry {
        id: "komga".to_owned(),
        origin: "https://example.org/plugin-komga".to_owned(),
        revision: revision.to_owned(),
        manifest: String::new(),
    };

    assert!(entry("8fa05ba718f70624f2c122f8c0371d47e6c90d0e").pins_a_commit());
    for revision in [
        "main",
        "v1.2.0",
        "8fa05ba7",
        "8fa05ba718f70624f2c122f8c0371d47e6c90d0z",
    ] {
        assert!(!entry(revision).pins_a_commit(), "{revision}");
    }
}

/// What signed a plugin is the key's name and its fingerprint.
#[test]
fn a_signer_is_named_with_its_fingerprint() {
    let named = Signing::new()
        .and_then(|signing| signing.key())
        .map(|key| signer(&key));

    assert!(
        named.as_deref().is_some_and(|named| named
            .starts_with("the catalogue's test key (sha256:")
            && named.len() == "the catalogue's test key (sha256:".len() + 64 + 1),
        "{named:?}"
    );
}
