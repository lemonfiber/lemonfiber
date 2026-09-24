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
fn recorded(plugin: &str, reached: Option<Reached>) -> Installed {
    Installed {
        plugin: plugin.to_owned(),
        version: "1.2.0".to_owned(),
        services: vec![Placed {
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
        }],
        provides: Vec::new(),
        contributions: Vec::new(),
        declared: lemonfiber_core::plugin::Declaration::default(),
        from: String::new(),
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
        contests: Vec::new(),
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
        Changing {
            path: "/opt/lemonfiber/stack/config/caddy/Caddyfile".to_owned(),
            puts: Puts::Region,
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
    }
}

mod installed;
mod proven;
mod reversed;
