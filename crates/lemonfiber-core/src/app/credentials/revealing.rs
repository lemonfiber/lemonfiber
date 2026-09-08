//! Printing one stored credential, because it is the operator's and they asked.
//!
//! Refusing outright would be this product deciding it knows better than the person
//! whose secrets these are, which it does not: an operator moving a service to
//! another machine, or signing in to qBittorrent's web UI by hand, has an ordinary
//! reason to need a value lemonfiber minted on their behalf and never showed them.
//!
//! What is owed instead is that it never happens by accident. Asking prints the
//! warning and not the value; asking again, having read it, prints the value. So a
//! credential can only reach a terminal through a request that says what it is
//! for — never as a side effect of listing what is held, and never from a surface
//! that merely renders whatever it is given.

use crate::app::targets::recorded_secret;
use crate::app::Ctx;
use crate::credential::{Held, Revealed, REVEALED, SHOULDER};

/// One credential, printed or explained.
///
/// Unconfirmed, the warning is the whole answer. Confirmed, the value comes with the
/// warning still attached, because the thing worth saying is as true afterwards as it
/// was before — the value is in the scrollback either way once it has been printed.
pub(super) fn reveal(ctx: &Ctx, held: &Held, confirmed: bool) -> Revealed {
    if !confirmed {
        return Revealed {
            name: held.name.clone(),
            value: None,
            warning: SHOULDER.to_owned(),
        };
    }
    match recorded_secret(ctx, &held.setting) {
        Some(value) => Revealed {
            name: held.name.clone(),
            value: Some(value),
            warning: REVEALED.to_owned(),
        },
        None => Revealed {
            name: held.name.clone(),
            value: None,
            warning: format!(
                "There is nothing recorded under {} for lemonfiber to show. Where a service \
                 minted its own key, lemonfiber holds a copy only once the stack has been \
                 seeded; the service's own settings hold it either way.",
                held.setting
            ),
        },
    }
}

/// What is said where nothing this stack holds answers to the name that was given.
///
/// A reveal rather than a rotation, because that is what was asked for. Reported as a
/// rotation it would put a replacement nobody asked for into the answer, render as one
/// on the terminal, and earn the exit code a failed replacement earns.
pub(super) fn nothing_by_that_name(credential: &str, known: &[String]) -> Revealed {
    Revealed {
        name: credential.to_owned(),
        value: None,
        warning: format!(
            "Nothing here is called `{credential}`. What is: {}.",
            known.join(", ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::reveal;
    use crate::app::targets::record_secret;
    use crate::config::Settings;
    use crate::credential::{Held, Origin, State, REVEALED, SHOULDER};
    use crate::test_support::a_context;

    /// An inventory line naming a credential recorded under a test's own setting.
    fn line(setting: &str) -> Held {
        Held {
            name: "qBittorrent web UI password".to_owned(),
            setting: setting.to_owned(),
            consumers: vec!["qBittorrent".to_owned()],
            location: "the settings file".to_owned(),
            origin: Origin::Lemonfiber,
            state: State::Active,
            fingerprint: None,
            advisory: None,
        }
    }

    /// A run whose settings file is a scratch path unique to the named test, so
    /// concurrent tests do not share one.
    fn keeping(name: &str) -> crate::app::Ctx {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-reveal-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        a_context()
            .settings(Settings {
                env_file: Some(dir.join(".env")),
                ..Settings::default()
            })
            .build()
    }

    /// A value built rather than written, so nothing reads it as a real credential.
    fn a_value() -> String {
        format!("{}{}", "the-", "recorded-value")
    }

    #[test]
    fn asking_once_prints_the_warning_and_not_the_value() {
        let ctx = keeping("once");
        let secret = a_value();
        record_secret(&ctx, "A_TEST_PASS", &secret);

        let revealed = reveal(&ctx, &line("A_TEST_PASS"), false);

        assert_eq!(revealed.value, None);
        assert_eq!(revealed.warning, SHOULDER);
    }

    #[test]
    fn asking_again_prints_the_value_with_the_warning_still_beside_it() {
        let ctx = keeping("again");
        let secret = a_value();
        record_secret(&ctx, "A_TEST_PASS", &secret);

        let revealed = reveal(&ctx, &line("A_TEST_PASS"), true);

        assert_eq!(revealed.value.as_deref(), Some(secret.as_str()));
        assert_eq!(revealed.warning, REVEALED);
    }

    #[test]
    fn a_credential_with_nothing_recorded_says_so_rather_than_showing_an_empty_value() {
        let ctx = keeping("nothing");

        let revealed = reveal(&ctx, &line("NOTHING_RECORDED_PASS"), true);

        assert_eq!(revealed.value, None);
        assert!(
            revealed.warning.contains("NOTHING_RECORDED_PASS"),
            "{}",
            revealed.warning
        );
    }
}
