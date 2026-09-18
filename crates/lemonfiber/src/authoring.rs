//! What a plugin author asks this binary, answered without a stack.
//!
//! Five reads and no verbs. Three are documents generated at build time from
//! lemonfiber's own types, so the answer is the same on a machine with nothing
//! installed as on one running everything — which is the whole of what an author
//! needs them to be. The fourth reads a manifest the author is writing. The fifth
//! asks each image's registry what it holds beside that image, and is the only one
//! here that reaches the network.
//!
//! Kept out of the dispatcher because none of it dispatches: there is no stack to
//! ask, nothing to decide, and a context to build would be a context nothing reached
//! through.
//!
//! **Every read returns what it would say rather than saying it.** What to show and
//! what to exit with is a decision, and a decision returned as a value is one a test
//! can hold; putting it in front of an operator is the edge's, and the edge is the
//! one place a test cannot follow. The registry arrives the same way, so the read
//! that reaches the network is driven here without one.

use std::path::{Path, PathBuf};

use lemonfiber::cli::PluginCommand;
use lemonfiber_core::plugin::Key;
use lemonfiber_core::ports::registry::Registry;

use crate::exit::FAILURE;
use crate::render;

/// What one of these reads comes to: what to say, and what to end with.
pub(crate) struct Answered {
    /// What the operator is shown, where the read has an answer.
    pub(crate) lines: Option<render::Lines>,
    /// What went wrong, where something did.
    pub(crate) fault: Option<String>,
    /// The code this run ends with.
    pub(crate) code: u8,
}

impl Answered {
    /// An answer, and a run that ends well.
    fn shown(lines: render::Lines) -> Self {
        Self {
            lines: Some(lines),
            fault: None,
            code: 0,
        }
    }

    /// An answer that is itself the bad news, so it is shown and still fails.
    ///
    /// The author's CI is asking whether this plugin would be installed, and the
    /// whole of why it would not is in the lines. Exiting non-zero without them
    /// would answer the question and withhold the reason.
    fn refused(lines: render::Lines) -> Self {
        Self {
            lines: Some(lines),
            fault: None,
            code: FAILURE,
        }
    }

    /// Nothing could be answered, and this is why.
    fn faulted(fault: String) -> Self {
        Self {
            lines: None,
            fault: Some(fault),
            code: FAILURE,
        }
    }

    /// A document this build could not write, which is this build's fault.
    fn unwritable() -> Self {
        Self::faulted("the published document could not be written".to_owned())
    }
}

/// Answer one of the five things a plugin author asks this binary.
///
/// The schema is always the document, because it is a thing an editor reads rather
/// than a listing a person does. The other four have a form for each.
pub(crate) async fn published(read: &PluginCommand, json: bool, asking: &dyn Registry) -> Answered {
    match read {
        PluginCommand::Schema => document_of(lemonfiber_core::plugin::schema().as_deref()),
        PluginCommand::ExtensionPoints if json => {
            document_of(lemonfiber_core::plugin::points().as_deref())
        }
        PluginCommand::ExtensionPoints => Answered::shown(render::plugin::points(
            &lemonfiber_core::plugin::extension_points(),
        )),
        PluginCommand::Capabilities => capabilities(json),
        PluginCommand::Claims { path } => claims(path, json),
        PluginCommand::Provenance { path, keys } => vouched_for(path, keys, json, asking).await,
    }
}

/// One committed document, going out exactly as it was written.
fn document_of(text: Option<&str>) -> Answered {
    text.map(render::plugin::document)
        .map_or_else(Answered::unwritable, Answered::shown)
}

/// The capability vocabulary, in whichever form was asked for.
///
/// Its own function because it is the one of the three documents that can refuse: it
/// is read against the stack this build pins, and a stack that disagreed with the
/// vocabulary would have failed generation long before here. Reported as this build's
/// own fault rather than the operator's, because it is.
fn capabilities(json: bool) -> Answered {
    match lemonfiber_core::plugin::capabilities() {
        Ok(_) if json => document_of(lemonfiber_core::plugin::vocabulary().ok().as_deref()),
        Ok(published) => Answered::shown(render::plugin::capabilities(&published)),
        Err(problem) => Answered::faulted(problem.to_string()),
    }
}

/// What one plugin's source claims, held to what this build publishes.
///
/// The one request under this word that reads something the operator has rather than
/// something lemonfiber published, and one of the two that can come back with an exit
/// status worth branching on: a manifest the vocabularies refuse, or a claim its own
/// recordings refute, is a plugin that would not be installed — which is what an
/// author's CI is asking.
///
/// Nothing is written and no service is asked anything. Every verdict is against the
/// recordings the plugin ships, which is what lets this run in a checkout with no
/// stack and no instance of the software anywhere.
fn claims(path: &Path, json: bool) -> Answered {
    let read = match lemonfiber_core::plugin::claimed(path) {
        Ok(read) => read,
        Err(unreadable) => return Answered::faulted(unreadable.to_string()),
    };
    render::plugin::claimed(&read, json).map_or_else(unrenderable, |lines| {
        if read.installable {
            Answered::shown(lines)
        } else {
            Answered::refused(lines)
        }
    })
}

/// What anybody has said about the images a plugin pins.
///
/// The one request under this word that reaches the network, and it reaches exactly
/// one kind of place: the registry each image is already pinned in. Nothing is
/// installed, nothing is written, and no service is asked anything.
///
/// A refused image exits non-zero, which is what a catalogue's CI is asking. An
/// unproven one does not: refusing every image nobody signed would refuse most of the
/// registry, and the answer an operator needs is which of the three it is rather than
/// a pass or a fail.
async fn vouched_for(path: &Path, keys: &[PathBuf], json: bool, asking: &dyn Registry) -> Answered {
    let manifest = match lemonfiber_core::plugin::read(path) {
        Ok(manifest) => manifest,
        Err(unreadable) => return Answered::faulted(unreadable.to_string()),
    };
    let held = match held_keys(keys) {
        Ok(held) => held,
        Err(fault) => return Answered::faulted(fault),
    };
    let read = lemonfiber_core::plugin::vouched(&manifest, &held, asking).await;
    render::plugin::vouched(&read, json).map_or_else(unrenderable, |lines| {
        if read.installable {
            Answered::shown(lines)
        } else {
            Answered::refused(lines)
        }
    })
}

/// What was read could not be written as JSON, which is this build's fault.
fn unrenderable() -> Answered {
    Answered::faulted("what was read could not be written as JSON".to_owned())
}

/// The keys an operator named, read from the paths they gave.
///
/// A key that cannot be read stops the run rather than being dropped: a verification
/// carried out against fewer keys than were asked for would report *unproven* about an
/// image somebody did sign, and an operator who supplied a key is owed the news that
/// it was not used.
///
/// # Errors
///
/// The first path that could not be read or did not carry a key, said as the operator
/// would want to read it.
fn held_keys(paths: &[PathBuf]) -> Result<Vec<Key>, String> {
    let mut held = Vec::new();
    for path in paths {
        let named = path.display().to_string();
        let text = std::fs::read_to_string(path)
            .map_err(|why| format!("{named} could not be read: {why}"))?;
        held.push(Key::from_pem(&named, &text).map_err(|unusable| unusable.to_string())?);
    }
    Ok(held)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use lemonfiber::cli::PluginCommand;
    use lemonfiber_core::ports::registry::Registry;
    use lemonfiber_fixtures::http::{Answer, Fake};

    use super::{document_of, held_keys, published, unrenderable, Answered};
    use crate::exit::FAILURE;

    /// A manifest nothing refuses, pinning one image and claiming nothing.
    const WHOLE: &str = r#"
schema_version = 1

[plugin]
id          = "kavita"
name        = "Kavita"
version     = "1.0.0"
description = "Reads comics in a browser"
without_it  = "Comics stay folders of images"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "kavita"
name        = "Kavita"
image       = "example.invalid/kavita"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.0.0"
criticality = "enhancing"
"#;

    /// The same manifest with the one pin that is not a content address.
    ///
    /// A digest that is not sixty-four hexadecimal characters is the refusal every
    /// other rule is built on: what was reviewed and what runs would be free to be
    /// different bytes.
    fn unpinned() -> String {
        WHOLE.replace(
            "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
            "sha256:not-a-digest",
        )
    }

    /// A plugin source written to a scratch directory.
    fn source(named: &str, manifest: &str) -> PathBuf {
        let at = std::env::temp_dir().join(format!(
            "lemonfiber-authoring-{}-{named}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&at);
        let _ = std::fs::create_dir_all(&at);
        let _ = std::fs::write(at.join("plugin.toml"), manifest);
        at
    }

    /// A file at a scratch path, holding whatever a case needs it to.
    fn written(named: &str, text: &str) -> PathBuf {
        let at = std::env::temp_dir().join(format!(
            "lemonfiber-authoring-{}-{named}",
            std::process::id()
        ));
        let _ = std::fs::write(&at, text);
        at
    }

    /// A path nothing has ever been written to.
    fn nowhere() -> PathBuf {
        std::env::temp_dir().join(format!(
            "lemonfiber-authoring-{}-absent",
            std::process::id()
        ))
    }

    /// A registry that answers every question the same way, over no network.
    fn answering(status: u16, body: &'static str) -> impl Registry {
        let reaching: Arc<dyn lemonfiber_core::ports::http::Http> =
            Fake::always(Answer::reply(status, body));
        lemonfiber_adapters::registry::Oci::new(reaching)
    }

    /// What one read came to, asked of a registry that offers nothing.
    async fn read(asked: PluginCommand, json: bool) -> Answered {
        published(&asked, json, &answering(404, "{}")).await
    }

    /// What was shown, or nothing where the read had no lines to show.
    fn said(answered: &Answered) -> String {
        answered
            .lines
            .as_ref()
            .map(super::render::Lines::text)
            .unwrap_or_default()
    }

    /// Each published document arrives in both forms, and neither is a fault.
    #[tokio::test]
    async fn every_document_this_build_publishes_has_a_form_for_each_reader() {
        /// Naming a read without making one, since a request is not `Clone`.
        type Asking = fn() -> PluginCommand;

        let asked: [(&str, Asking); 3] = [
            ("schema", || PluginCommand::Schema),
            ("extension-points", || PluginCommand::ExtensionPoints),
            ("capabilities", || PluginCommand::Capabilities),
        ];
        let mut seen = 0_usize;
        for (named, one) in asked {
            for json in [true, false] {
                let answered = read(one(), json).await;
                assert_eq!(answered.code, 0, "{named} at json={json}");
                assert!(answered.fault.is_none(), "{named} at json={json}");
                assert!(!said(&answered).is_empty(), "{named} at json={json}");
                seen += 1;
            }
        }
        assert_eq!(seen, 6, "the documents were not put in front of this");
    }

    /// The schema is the document whichever form was asked for.
    ///
    /// It is a thing an editor reads rather than a listing a person does, so unlike
    /// the other two it has no second form to fall back to.
    #[tokio::test]
    async fn the_schema_is_the_same_document_either_way() {
        let plain = said(&read(PluginCommand::Schema, false).await);
        let machine = said(&read(PluginCommand::Schema, true).await);
        assert_eq!(plain, machine);
        assert!(plain.contains("schema_version"), "{plain}");
    }

    /// The two listings are listings for a person and documents for a parser.
    #[tokio::test]
    async fn a_listing_reads_as_prose_and_a_document_reads_as_json() {
        let listed = said(&read(PluginCommand::ExtensionPoints, false).await);
        let document = said(&read(PluginCommand::ExtensionPoints, true).await);
        assert_ne!(listed, document);
        assert!(document.starts_with('{'), "{document}");
    }

    #[tokio::test]
    async fn a_plugin_nothing_refuses_is_read_and_the_run_ends_well() {
        let at = source("whole", WHOLE);
        let answered = read(PluginCommand::Claims { path: at }, false).await;
        assert_eq!(answered.code, 0, "{:?}", answered.fault);
        assert!(answered.fault.is_none());
        assert!(said(&answered).contains("kavita"), "{}", said(&answered));
    }

    /// A refused manifest is shown and still fails.
    ///
    /// The author's CI is asking whether this would be installed, and the whole of
    /// why it would not is in the lines — exiting non-zero without them would answer
    /// the question and withhold the reason.
    #[tokio::test]
    async fn a_plugin_that_would_not_install_says_why_and_still_fails() {
        let at = source("unpinned", &unpinned());
        let answered = read(PluginCommand::Claims { path: at }, false).await;
        assert_eq!(answered.code, FAILURE);
        assert!(answered.fault.is_none(), "{:?}", answered.fault);
        assert!(!said(&answered).is_empty(), "the reason was withheld");
    }

    /// Pointing at the wrong directory is a different thing from a refused manifest.
    #[tokio::test]
    async fn a_path_holding_no_manifest_is_a_fault_rather_than_a_verdict() {
        for asked in [
            PluginCommand::Claims { path: nowhere() },
            PluginCommand::Provenance {
                path: nowhere(),
                keys: Vec::new(),
            },
        ] {
            let answered = read(asked, false).await;
            assert_eq!(answered.code, FAILURE);
            assert!(answered.lines.is_none());
            let fault = answered.fault.unwrap_or_default();
            assert!(fault.contains("plugin.toml"), "{fault}");
        }
    }

    /// An image nobody signed does not stop an install, and says so in words.
    #[tokio::test]
    async fn an_image_no_registry_offers_a_signature_for_is_unproven_and_allowed() {
        let at = source("unproven", WHOLE);
        let answered = read(
            PluginCommand::Provenance {
                path: at,
                keys: Vec::new(),
            },
            false,
        )
        .await;
        assert_eq!(answered.code, 0, "{:?}", answered.fault);
        let said = said(&answered);
        assert!(said.contains("unproven"), "{said}");
        assert!(said.contains("stops an install"), "{said}");
    }

    /// The same read for a parser is the document, not the prose.
    #[tokio::test]
    async fn what_was_vouched_for_is_a_document_where_a_script_asked() {
        let at = source("vouched-json", WHOLE);
        let said = said(
            &read(
                PluginCommand::Provenance {
                    path: at,
                    keys: Vec::new(),
                },
                true,
            )
            .await,
        );
        assert!(said.contains(r#""provenance": "unproven""#), "{said}");
    }

    /// A key that cannot be read stops the run rather than being dropped.
    ///
    /// A check carried out against fewer keys than were asked for would report
    /// *unproven* about an image somebody did sign.
    #[test]
    fn a_key_this_build_cannot_read_stops_the_run_naming_it() {
        let absent = held_keys(&[nowhere()]);
        assert!(absent
            .as_ref()
            .err()
            .is_some_and(|why| why.contains("could not be read")));

        let nonsense = held_keys(&[written("not-a-key", "hello")]);
        assert!(nonsense
            .as_ref()
            .err()
            .is_some_and(|why| why.contains("P-256")));

        assert!(held_keys(&[]).is_ok_and(|held| held.is_empty()));
    }

    /// A key on the P-256 curve is read and named as the operator named it.
    #[test]
    fn a_key_an_operator_named_is_read_under_the_path_they_gave() {
        let at = written("a-key", PUBLIC_KEY);
        let held = held_keys(std::slice::from_ref(&at));
        assert_eq!(
            held.ok()
                .and_then(|held| held.first().map(|key| key.named.clone())),
            Some(at.display().to_string())
        );
    }

    /// A public key on the curve this build verifies against.
    const PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----\n\
        MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEXO0Zk0m0jt9dqnbvJx6tQYFThLEC\n\
        E9HmhZsMHLTKQ2u/0O4bnfsX+zEr1Nru7O6Yf+MbtXeJ+rNJ4BBBzbf9bA==\n\
        -----END PUBLIC KEY-----\n";

    /// The two faults that are this build's own, said as this build's own.
    ///
    /// Reached by calling them rather than by driving a document that fails to
    /// render, because neither can be made to happen from outside: a generated
    /// document that would not serialise fails long before an operator asks for it.
    /// What is worth holding is the sentence each gives, since that sentence is the
    /// whole of what reaches somebody when it does.
    #[test]
    fn a_document_this_build_could_not_write_is_reported_as_this_builds_fault() {
        let unwritten = document_of(None);
        assert_eq!(unwritten.code, FAILURE);
        assert!(unwritten.lines.is_none());
        assert!(unwritten
            .fault
            .is_some_and(|why| why.contains("could not be written")));

        let unread = unrenderable();
        assert_eq!(unread.code, FAILURE);
        assert!(unread
            .fault
            .is_some_and(|why| why.contains("could not be written as JSON")));
    }

    /// A document that did render is shown and the run ends well.
    #[test]
    fn a_document_that_rendered_is_shown_without_a_fault() {
        let written = document_of(Some("{}"));
        assert_eq!(written.code, 0);
        assert!(written.fault.is_none());
        assert_eq!(
            written.lines.as_ref().map(super::render::Lines::text),
            Some("{}".to_owned())
        );
    }

    /// The scratch paths are the ones these cases wrote, and nowhere else.
    #[test]
    fn nothing_here_reaches_outside_a_scratch_directory() {
        for at in [
            source("scoped", WHOLE),
            written("scoped-file", "x"),
            nowhere(),
        ] {
            assert!(at.starts_with(std::env::temp_dir()), "{}", at.display());
        }
    }

    /// The path a read is given is the plugin's, and a file inside it reads the same.
    #[tokio::test]
    async fn a_directory_and_the_manifest_inside_it_are_the_same_plugin() {
        let at = source("either-way", WHOLE);
        let directory = read(PluginCommand::Claims { path: at.clone() }, true).await;
        let file = read(
            PluginCommand::Claims {
                path: at.join("plugin.toml"),
            },
            true,
        )
        .await;
        assert_eq!(said(&directory), said(&file));
        assert_eq!(directory.code, file.code);
    }

    /// A path that is neither is refused, and the refusal names what was looked for.
    #[test]
    fn a_manifest_this_build_cannot_read_is_refused_rather_than_half_read() {
        let at = source("half", "schema_version = 1\n");
        let read = lemonfiber_core::plugin::read(Path::new(&at));
        assert!(read.is_err(), "a manifest missing everything was read");
    }
}
