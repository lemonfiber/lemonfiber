use lemonfiber_core::app::putting_back::Reversal;
use lemonfiber_core::journal::{Action, Undo};
use lemonfiber_core::plugin::{
    Changing, Evidence, Install, Installed, Installs, Overriding, Placed, Proving, Puts, Reached,
    Removal, Restored, Unfilled, Update, Verdict, Verification,
};

use super::{installs, stood};
use lemonfiber_core::doctor::{Category, Finding, Verdict as Checked};

/// One plugin's record, as an install settles it.
///
/// The library is mounted for a service that is reached and not for one that is
/// not, which is not a rule — it is the pair of shapes this renderer draws
/// differently, arranged so one listing exercises both.
pub(super) fn recorded(plugin: &str, reached: Option<Reached>) -> Installed {
    Installed {
        granted: Vec::new(),
        plugin: plugin.to_owned(),
        version: "1.2.0".to_owned(),
        name: None,
        description: None,
        services: vec![Placed {
            fields: lemonfiber_core::plugin::Fields::default(),
            service: plugin.to_owned(),
            image: format!("example.invalid/{plugin}"),
            digest: "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
                .to_owned(),
            tag: "1.11.0".to_owned(),
            config_path: "/app/data".to_owned(),
            takes_data: reached.is_some(),
            reached,
            provides: Vec::new(),
            name: "Komga".to_owned(),
            description: "Reads comics".to_owned(),
            icon: None,
            hidden: false,
            health: None,
            listens: None,
            contract: None,
            media_types: Vec::new(),
            networks: Vec::new(),
            speaks: Vec::new(),
            fronts: None,
            upstream: std::collections::BTreeMap::new(),
            native: None,
            replaces: None,
            shape: None,
            asks: Vec::new(),
            reaches: None,
            asks_for: None,
            starting: None,
        }],
        provides: Vec::new(),
        contributions: Vec::new(),
        run_proofs: Vec::new(),
        request_proofs: Vec::new(),
        declared: lemonfiber_core::plugin::Declaration::default(),
        recipes: Vec::new(),
        from: String::new(),
        revision: String::new(),
        signed: String::new(),
        manifest: String::new(),
        tree: String::new(),
        installed_at: String::new(),
    }
}

/// What an install came to, with the three accounts the case under test needs.
///
/// A helper rather than five literals, so that a field added to the report is one
/// edit here and every case keeps saying what it was written to say.
fn install(would: Installed, recorded: bool) -> Install {
    Install {
        would,
        recorded,
        changes: Vec::new(),
        proofs: Vec::new(),
        against: None,
        verified: None,
        overrides: Vec::new(),
        reversed: None,
        asks: Vec::new(),
        contests: Vec::new(),
        recipes_ran: Vec::new(),
        taking: Vec::new(),
        granting: Vec::new(),
        sharing: Vec::new(),
        waiting: Vec::new(),
        secured: None,
    }
}

/// What an install writes, as the report states it.
fn writes() -> Vec<Changing> {
    vec![
        Changing {
            path: "/opt/lemonfiber/stack/config/komga".to_owned(),
            puts: Puts::Directory,
        },
        Changing {
            path: "/opt/lemonfiber/stack/compose/plugins/komga.yml".to_owned(),
            puts: Puts::Document,
        },
    ]
}

/// One proof, as the report states it.
fn proof() -> Proving {
    Proving {
        proof: "answers".to_owned(),
        establishes: "the library API answers".to_owned(),
        of: Some("komga".to_owned()),
        asks: "GET /api/v1/libraries".to_owned(),
        why: "a plugin whose service does not answer is not installed".to_owned(),
        came_to: None,
    }
}

/// A household service, as the record carries one.
fn household() -> Reached {
    Reached::Household {
        port: 25600,
        hostname: "comics".to_owned(),
        group: Some("Library".to_owned()),
    }
}

/// An update built for the page: from one version to the next, with whatever it
/// came to.
fn moving(recorded: bool, restored: Option<Restored>, stopped: Option<&str>) -> Installs {
    let mut next = self::recorded("komga", None);
    next.version = "1.3.0".to_owned();
    Installs {
        demand: None,
        nonconforming: Vec::new(),
        proof: None,
        rehearsed: false,
        agreement: None,
        installed: vec![self::recorded("komga", None)],
        install: None,
        removal: None,
        update: Some(Box::new(Update {
            plugin: "komga".to_owned(),
            from: "1.2.0".to_owned(),
            to: "1.3.0".to_owned(),
            interrupts: vec!["komga".to_owned()],
            went_back: Reversal {
                rehearsed: !recorded && restored.is_none(),
                ..Reversal::default()
            },
            install: Install {
                reversed: restored.as_ref().map(|_| Reversal::default()),
                ..install(next, recorded)
            },
            stopped: stopped.map(str::to_owned),
            restored,
        })),
        substituted: Vec::new(),
        sources: Vec::new(),
        unvetted: None,
        guards: Vec::new(),
    }
}

/// An adapter writing its upstream's files is said to, in the tense the install or the
/// update is in.
#[test]
fn an_adapter_writing_its_upstreams_files_is_said_to() {
    let shared = vec![lemonfiber_core::plugin::Sharing {
        adapter: "komga".to_owned(),
        upstream: "komga-server".to_owned(),
        at: "/upstream".to_owned(),
    }];
    let installing = |recorded: bool| {
        let mut report = moving(recorded, None, None);
        report.update = None;
        report.install = Some(Box::new(Install {
            sharing: shared.clone(),
            ..install(self::recorded("komga", None), recorded)
        }));
        installs(&report).text()
    };
    let updating = {
        let mut report = moving(true, None, None);
        if let Some(update) = report.update.as_mut() {
            update.install.sharing.clone_from(&shared);
        }
        installs(&report).text()
    };
    let line = "komga writes komga-server's own files, its whole configuration directory \
                mounted at /upstream";
    for said in [installing(true), updating] {
        assert!(
            said.contains("The files of another service it writes:") && said.contains(line),
            "{said}"
        );
    }
    let rehearsed = installing(false);
    assert!(
        rehearsed.contains("The files of another service it would write:"),
        "{rehearsed}"
    );
    assert!(
        !installs(&moving(true, None, None))
            .text()
            .contains("files of another service"),
        "nothing is said where nothing is shared"
    );
}

/// What asking the egress guard to hold its control credential came to is said on an
/// install and an update alike.
#[test]
fn a_guard_restarted_to_take_up_its_control_credential_is_said_to_have_been() {
    let said = "gluetun restarted to take up the control credential it was handed";
    let installing = {
        let mut report = moving(true, None, None);
        report.update = None;
        report.install = Some(Box::new(Install {
            secured: Some(said.to_owned()),
            ..install(self::recorded("komga", None), true)
        }));
        installs(&report).text()
    };
    let updating = {
        let mut report = moving(true, None, None);
        if let Some(update) = report.update.as_mut() {
            update.install.secured = Some(said.to_owned());
        }
        installs(&report).text()
    };
    for text in [installing, updating] {
        assert!(text.contains(said), "{text}");
    }
    assert!(!installs(&moving(true, None, None))
        .text()
        .contains("control credential"));
}

mod installed;
mod proven;
mod reversed;
