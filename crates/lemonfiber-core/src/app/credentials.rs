//! The one place an operator asks what secrets this stack holds.
//!
//! Three asks under one word, and all three answer with the inventory as it now
//! stands: reading it, replacing one line of it, or printing one value. An operator
//! who has just rotated something wants to see the inventory that rotation produced,
//! not a receipt they then have to check against one.
//!
//! Naming a credential is a lookup rather than an identifier, on purpose. What the
//! inventory prints is what the operator types back — matched loosely enough that
//! `qbittorrent` finds the web UI password, because a product that prints one string
//! and demands another is one that has to be read twice.
//!
//! Nothing here can answer without the stack, and that is deliberate. Most of the
//! secrets in this stack belong to services, so a manifest that could not be read
//! would leave those reading as none at all — which is a claim, not a gap.

mod reading;
mod revealing;
mod rotating;

use super::targets::project_directory;
use super::{Ctx, Outcome, Problem};
use crate::app::command::Asking;
use crate::credential::{Held, Inventory, Rotation, Settled};
use crate::error::Diagnose;

/// What was asked about the credentials this stack holds.
///
/// # Errors
///
/// Returns the [`Problem`] for a stack that could not be read.
pub(super) async fn answer(ctx: &Ctx, asked: Asking) -> Result<Outcome, Box<Problem>> {
    let manifest = ctx
        .stack
        .manifest()
        .map_err(|err| Box::new(err.problem()))?;
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let held = reading::taken(ctx, &manifest.services, project.as_deref()).await;

    let inventory = match asked {
        Asking::Read => Inventory::of(held),
        Asking::Reveal {
            credential,
            confirmed,
        } => showing(ctx, held, &credential, confirmed),
        Asking::Rotate { credential } => {
            replacing(
                ctx,
                held,
                &credential,
                &manifest.services,
                project.as_deref(),
            )
            .await
        }
    };
    Ok(Outcome::Credentials(inventory))
}

/// The inventory with one value printed, or with the reason there is nothing to print.
fn showing(ctx: &Ctx, held: Vec<Held>, credential: &str, confirmed: bool) -> Inventory {
    let revealed = match named(&held, credential) {
        Some(found) => revealing::reveal(ctx, found, confirmed),
        None => revealing::nothing_by_that_name(credential, &named_ones(&held)),
    };
    Inventory::of(held).showing(revealed)
}

/// The inventory as it stands after a rotation, and what became of that rotation.
async fn replacing(
    ctx: &Ctx,
    held: Vec<Held>,
    credential: &str,
    services: &[lemonfiber_manifest::Service],
    project: Option<&std::path::Path>,
) -> Inventory {
    let Some(found) = named(&held, credential) else {
        let known = named_ones(&held);
        return Inventory::of(held).after(unknown(credential, &known));
    };
    let rotated = rotating::rotate(ctx, found, services, project).await;
    // Read again, because a landed replacement has changed what the answer is and an
    // inventory taken before it would report the state the rotation just left behind.
    let after = reading::taken(ctx, services, project).await;
    Inventory::of(after).after(rotated)
}

/// The line an operator's words name, where they name one.
///
/// Matched on the name it is printed under and on the setting it is recorded as, and
/// loosely: a name typed without its capitals, or a service typed without the words
/// around it, is the same request. Ambiguity resolves to the first match in the order
/// the inventory lists, which is the order it was printed in.
fn named<'a>(held: &'a [Held], asked: &str) -> Option<&'a Held> {
    let asked = asked.trim().to_lowercase();
    if asked.is_empty() {
        return None;
    }
    held.iter()
        .find(|one| one.name.to_lowercase() == asked || one.setting.to_lowercase() == asked)
        .or_else(|| {
            held.iter()
                .find(|one| one.name.to_lowercase().contains(&asked))
        })
}

/// Every name that would have been accepted, in the order they are listed.
fn named_ones(held: &[Held]) -> Vec<String> {
    held.iter().map(|one| one.name.clone()).collect()
}

/// A rotation that never started, because nothing here answers to that name.
fn unknown(credential: &str, known: &[String]) -> Rotation {
    Rotation::stopped(
        credential,
        Settled::Unknown {
            known: known.to_vec(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{named, named_ones, unknown};
    use crate::credential::{Held, Origin, Settled, State};

    /// Two lines to look up between.
    fn inventory() -> Vec<Held> {
        vec![
            line("qBittorrent web UI password", "QBITTORRENT_PASSWORD"),
            line("Sonarr API key", "SONARR_API_KEY"),
        ]
    }

    fn line(name: &str, setting: &str) -> Held {
        Held {
            name: name.to_owned(),
            setting: setting.to_owned(),
            consumers: Vec::new(),
            location: "somewhere".to_owned(),
            origin: Origin::Lemonfiber,
            state: State::Active,
            fingerprint: None,
            advisory: None,
        }
    }

    #[test]
    fn a_name_typed_exactly_as_it_is_printed_finds_its_line() {
        let found = named(&inventory(), "qBittorrent web UI password");

        assert_eq!(
            found.map(|one| one.setting.as_str()),
            Some("QBITTORRENT_PASSWORD")
        );
    }

    #[test]
    fn the_setting_it_is_recorded_under_finds_it_too() {
        let found = named(&inventory(), "sonarr_api_key");

        assert_eq!(found.map(|one| one.name.as_str()), Some("Sonarr API key"));
    }

    #[test]
    fn a_word_out_of_the_name_finds_it_without_the_rest_of_the_sentence() {
        let found = named(&inventory(), "  QBITTORRENT  ");

        assert_eq!(
            found.map(|one| one.name.as_str()),
            Some("qBittorrent web UI password")
        );
    }

    #[test]
    fn nothing_typed_names_nothing_rather_than_the_first_line() {
        assert!(named(&inventory(), "   ").is_none());
    }

    #[test]
    fn a_name_nothing_answers_to_finds_nothing() {
        assert!(named(&inventory(), "the wifi password").is_none());
    }

    #[test]
    fn a_name_nothing_answers_to_is_told_what_would_have_been_accepted() {
        let held = inventory();
        let stopped = unknown("the wifi password", &named_ones(&held));

        assert!(stopped.kept_the_existing());
        assert!(stopped.consumers.is_empty());
        assert_eq!(
            stopped.settled,
            Settled::Unknown {
                known: vec![
                    "qBittorrent web UI password".to_owned(),
                    "Sonarr API key".to_owned(),
                ],
            }
        );
    }
}
