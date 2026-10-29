use super::settling::costs;
use lemonfiber_manifest::Manifest;

const STACK: &str = lemonfiber_bundled::STACK;

/// What the stack this repository ships would say about these services.
fn said(waiting: &[&str]) -> Option<String> {
    let named: Vec<String> = waiting.iter().map(|id| (*id).to_owned()).collect();
    Manifest::from_toml(STACK)
        .ok()
        .map(|manifest| costs(&manifest, &named))
}

/// Both ways of starting make the door's certificate before they spawn, because there
/// are two, and the door will not serve without one.
///
/// Pinned by the call rather than by behaviour, since neither path can be run here
/// without a container to start.
#[test]
fn both_ways_of_starting_make_the_doors_certificate_first() {
    const WAITED: &str = include_str!("../engine.rs");
    const STREAMED: &str = include_str!("../engine/streaming.rs");
    for (path, source) in [("engine.rs", WAITED), ("engine/streaming.rs", STREAMED)] {
        assert!(
            source.contains("door::made_before_start(ctx, &manifest)"),
            "{path} starts services without making the door's certificate"
        );
        assert!(
            source.contains("encrypting::made_before_start(ctx, &manifest)"),
            "{path} starts services without making the media server's internal certificate"
        );
        assert!(
            source.contains("declared::made_before_start(ctx, &manifest)?"),
            "{path} starts services without making the certificates they declare"
        );
    }
}

/// Both ways of starting ask the same two things first, because there are two.
///
/// The same hazard the certificate above is pinned against, on two questions asked at
/// moments nobody is watching. A start recorded on only the waited-on path leaves
/// the next boot bringing back whatever form the *other* path last named; a data
/// location proven present on only that path leaves the one an operator actually
/// types building a second library on the system disk.
///
/// Pinned by the call rather than by behaviour, since neither path can be run here
/// without a container to start. The production half of each file is what is read,
/// so the name appearing in this very test does not satisfy it.
#[test]
fn both_ways_of_starting_ask_the_same_things_before_they_spawn() {
    const WAITED: &str = include_str!("../engine.rs");
    const STREAMED: &str = include_str!("../engine/streaming.rs");
    for (path, source) in [("engine.rs", WAITED), ("engine/streaming.rs", STREAMED)] {
        // Split at the test module rather than at the first `#[cfg(test)]`: one of
        // these files carries a test-only re-export near its imports, and splitting
        // there would read nine lines of `use` and call them the whole file.
        let production = source
            .split_once("mod tests {")
            .map_or(source, |(before, _)| before);
        assert!(
            production.contains("autostart::noted(ctx, "),
            "{path} starts services without recording what was asked for"
        );
        assert!(
            production.contains("grounded::grounded(ctx, "),
            "{path} starts services without proving the data location is there"
        );
    }
}

#[test]
fn what_a_service_is_for_is_said_in_the_stacks_own_words() {
    assert_eq!(
        said(&["jellyfin"]).as_deref(),
        Some("What that costs, while it lasts: jellyfin — Files on disk, no way to watch them."),
        "the manifest's sentence, not one written here"
    );
}

#[test]
fn several_services_are_said_together_in_the_order_the_stack_declares_them() {
    let declared: Vec<String> = Manifest::from_toml(STACK)
        .map(|manifest| manifest.services.into_iter().map(|one| one.id).collect())
        .unwrap_or_default();
    let at = |id: &str| declared.iter().position(|one| one == id);
    let mut asked = ["seerr", "jellyfin"];
    asked.sort_by_key(|id| std::cmp::Reverse(at(id)));
    let said = said(&asked);
    let mut stacked = asked;
    stacked.sort_by_key(|id| at(id));
    assert!(
        said.as_ref().is_some_and(|said| {
            said.find(stacked[0])
                .zip(said.find(stacked[1]))
                .is_some_and(|(first, second)| first < second)
        }),
        "asked for in one order, reported in the stack's: {said:?}"
    );
}

/// A stack that says nothing about a service contributes nothing, rather than a
/// sentence with a hole in it. Reached here by naming a service the stack does
/// not declare, which is the same silence as one that describes itself as "".
#[test]
fn a_service_the_stack_says_nothing_about_costs_no_words() {
    assert_eq!(
        said(&["not-a-service-this-stack-declares"]).as_deref(),
        Some("")
    );
}

/// A failed start against an engine that will not answer leaves the report as the
/// start's own failure: no service read, and no condition made up for them.
///
/// Driven here rather than through a dispatched start, because a start meets an engine
/// that is not there at its preflight and never reaches the read at all.
#[tokio::test]
async fn a_start_that_failed_reads_nothing_from_an_engine_that_is_not_there() {
    let rehearsed = crate::test_support::a_context().build().rehearsing();
    let started = crate::app::dispatch(
        crate::app::Command::Up {
            forms: vec!["library".to_owned()],
        },
        &rehearsed,
    )
    .await;
    let Ok(crate::app::Outcome::Lifecycle(mut report)) = started else {
        unreachable!("a rehearsed start answers with its plan: {started:?}");
    };
    let gone = crate::test_support::a_context()
        .engine(std::sync::Arc::new(
            lemonfiber_fixtures::support::Reporting::absent(),
        ))
        .build();
    let manifest = Manifest::from_toml(STACK);

    if let Ok(manifest) = &manifest {
        super::fell_short_into(&gone, manifest, &mut report).await;
    }

    assert!(manifest.is_ok());
    assert!(report.services.is_empty(), "{:?}", report.services);
    assert_eq!(report.condition, None);
}

/// What an action is aimed at is what it names, or everything its plan holds where it
/// names nothing — the same reading Compose gives it, asked once for every action.
#[test]
fn an_action_is_aimed_at_what_it_names_or_else_at_its_plan() {
    use crate::stack::compose::Action;

    let plan = crate::stack::closure::Plan {
        forms: Vec::new(),
        profiles: std::collections::BTreeSet::new(),
        services: vec!["gluetun".to_owned(), "qbittorrent".to_owned()],
        dropped: Vec::new(),
        filtered: Vec::new(),
        footprint: crate::stack::closure::Footprint::default(),
        running: crate::stack::closure::Running::Unasked,
    };
    let one = vec!["gluetun".to_owned()];
    for action in [
        Action::Start(one.clone()),
        Action::Stop(one.clone()),
        Action::Remove(one.clone()),
        Action::Restart(one.clone()),
    ] {
        assert_eq!(super::addressed(&action, &plan), one, "{action:?}");
    }
    for action in [
        Action::Up,
        Action::Down,
        Action::Start(Vec::new()),
        Action::Stop(Vec::new()),
        Action::Remove(Vec::new()),
        Action::Restart(Vec::new()),
        Action::Pull(Vec::new()),
        Action::Config,
    ] {
        assert_eq!(
            super::addressed(&action, &plan),
            plan.services,
            "{action:?}"
        );
    }
}

/// A start whose change to the operator's overlay cannot be journalled starts nothing,
/// and says why.
#[tokio::test]
async fn an_overlay_change_the_journal_cannot_hold_starts_nothing() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("overlay-unjournalled");
    let config = dir.join("config");
    let _ = std::fs::create_dir_all(config.join("journal.jsonl"));
    let _ = std::fs::write(config.join("overlay.yml"), "services: {}\n");
    let ctx = crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(config.join(".env")),
            stack_dir: Some(dir.join("data").join("stack")),
            ..crate::config::Settings::default()
        })
        .build();
    let started = crate::app::dispatch(
        crate::app::Command::Up {
            forms: vec!["library".to_owned()],
        },
        &ctx,
    )
    .await;
    let _ = std::fs::remove_dir_all(&dir);

    let Err(problem) = started else {
        unreachable!("a journal that cannot be written starts nothing: {started:?}");
    };
    assert!(problem.summary.contains("journal"), "{problem:?}");
}

mod sweeping;

#[test]
fn a_part_a_choice_passed_over_is_not_among_what_starting_everything_starts() {
    let dir = crate::app::fixtures::scratch("engine-replaced").kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join(".env"),
        "LEMONFIBER_FILLS=indexer.search=nzbhydra2\n",
    );
    let ctx = crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(dir.join(".env")),
            ..crate::config::Settings::default()
        })
        .build();

    let services = super::resolved(&ctx, &[])
        .map(|(_, plan)| plan.services)
        .unwrap_or_default();

    assert!(
        !services.iter().any(|one| one == "prowlarr"),
        "{services:?}"
    );
    assert!(
        services.iter().any(|one| one == "nzbhydra2"),
        "{services:?}"
    );
}
