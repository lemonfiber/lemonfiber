//! Which machine a run is aimed at is worked out once, and only once.
//!
//! The behaviour is asserted next door, in the core, where the invocation and the
//! engine client are built from one settings field and shown to agree. This is the
//! other half of that claim, and it is the half a behaviour test cannot make: that
//! there is no *second* resolution anywhere to disagree with the first.
//!
//! That is not hypothetical. It is exactly the shape the shipped defect had — the
//! client library read `DOCKER_HOST` for itself, Compose inherited the environment,
//! and the two agreed on every machine where nobody had set a context. A test that
//! only compared two resolutions would have passed throughout.
//!
//! So the rule is about where a name may appear rather than about what a run does:
//! the environment is read in the adapter that resolves the target, and everything
//! else takes the answer. A guard of this shape is what `nothing_is_carried_through`
//! holds for the migration, and for the same reason.

use crate::source_tree::{production, sources};

/// The one file allowed to read which engine this machine is pointed at.
const RESOLVER: &str = "lemonfiber-adapters/src/docker/context.rs";

/// The names Docker's own tooling reads to decide which daemon it means.
///
/// A second reader of any of these is a second answer to the same question, and the
/// two only differ where it matters — on the machine where a context is set.
const CHOOSING: [&str; 3] = ["DOCKER_HOST", "DOCKER_CONTEXT", "DOCKER_TLS_VERIFY"];

/// Nothing but the resolver asks the environment which engine this run operates.
#[test]
fn only_one_place_works_out_which_engine_this_run_operates() {
    let mut elsewhere: Vec<String> = Vec::new();
    for (path, text) in sources() {
        let where_it_lives = path.to_string_lossy().replace('\\', "/");
        if !where_it_lives.contains("/src/") || where_it_lives.ends_with(RESOLVER) {
            continue;
        }
        let shipped = production(&text);
        for name in CHOOSING {
            // Quoted, because the name has to be spelled the way a lookup spells it
            // to be one. A sentence that merely mentions the variable is prose.
            if shipped.contains(&format!("\"{name}\"")) {
                elsewhere.push(format!("{where_it_lives}: {name}"));
            }
        }
    }
    assert!(
        elsewhere.is_empty(),
        "the engine a run operates is resolved in {RESOLVER} and nowhere else — a \
         second reader is a second answer, and the two differ only on the machine \
         where a context is set: {elsewhere:?}"
    );
}

/// The surface builds every engine seam from the one answer it was given.
///
/// Both of them, which is the part that was quietly wrong: the image listing is an
/// engine read like any other and went through a client of its own, so on a remote
/// context one more reader stayed behind on the laptop.
#[test]
fn the_surface_builds_both_engine_seams_from_the_one_answer() {
    let sources = sources();
    let wiring = sources.iter().find(|(path, _)| {
        path.to_string_lossy()
            .replace('\\', "/")
            .ends_with("lemonfiber/src/context.rs")
    });
    let Some(context) = wiring.map(|(_, text)| production(text)) else {
        unreachable!("the binary has a context module");
    };

    for built in [
        "Daemon::reaching(settings.docker.clone())",
        "live_reaching(&settings.docker)",
    ] {
        assert!(
            context.contains(built),
            "the surface no longer builds its engine seams from the resolved target: \
             `{built}` is not in context.rs"
        );
    }
    assert!(
        !context.contains("Daemon::local()"),
        "a seam built from this machine's defaults is a seam that ignores the context"
    );
}
