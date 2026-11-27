//! A media server standing in, proved through what it serves: the library the seed
//! furnished it holds a title written there once the walkthrough has it look again, and,
//! where it holds the household's members too, a member invited into it holds that title.

use std::path::Path;
use std::time::Duration;

use super::core::{strings_under, Core};
use super::{
    fixture, providing, started, step, undone, waited_every, Asked, Substitution, IDENTIFIES,
    LOOKING, MEMBER,
};

/// What setup is told to fetch over: nothing, which makes it a stack serving a library
/// alone.
const NOTHING_FETCHED: &str = "none";

/// How long a title written into a library may take to be held.
const HELD_WITHIN: Duration = Duration::from_secs(5 * 60);

/// Whether what `core` reads as held, for `whom`, names the fixture.
fn holds(core: &Core, whom: &[&str]) -> bool {
    let mut asked = vec!["held"];
    asked.extend(whom);
    strings_under(&core.run(&asked, "").said, "title")
        .iter()
        .any(|title| title == fixture::TITLE)
}

/// Run `substitution`, a media server, live through a stack this build sets up beneath
/// `root` for `asked`'s media type alone, chosen as the identity source too where it is
/// one, since installing it contests the stack's.
///
/// # Panics
///
/// At the first step that does not come to what it should, naming it and what was said.
pub fn run(root: &Path, asked: &Asked, substitution: &Substitution) {
    let identifies = providing(&substitution.plugin, IDENTIFIES).is_some();
    let substitution = Substitution {
        also: identifies
            .then(|| IDENTIFIES.to_owned())
            .into_iter()
            .collect(),
        ..substitution.clone()
    };
    let Some(core) = started(
        root,
        asked.forms,
        (NOTHING_FETCHED, &[]),
        Some(&substitution),
    ) else {
        return;
    };

    let library = super::library(root, asked.media);
    assert!(
        library.is_dir(),
        "the seed furnished no library for {} at {}",
        asked.media,
        library.display()
    );
    let written = fixture::written(&library, asked.media);
    assert!(written.is_ok(), "{written:?}");
    assert!(
        waited_every(HELD_WITHIN, LOOKING, || {
            let _ = core.run(&["walkthrough", fixture::TITLE], "");
            holds(&core, &["--defaults"])
        }),
        "{} never held {written:?}",
        substitution.plugin
    );

    if identifies {
        step(&core.run(&["invite", MEMBER], ""), "invite");
        assert!(
            waited_every(HELD_WITHIN, LOOKING, || holds(&core, &["--member", MEMBER])),
            "{MEMBER} never held {written:?} in {}",
            substitution.plugin
        );
    }

    undone(&core);
}
