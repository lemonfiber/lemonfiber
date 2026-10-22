//! A data root the setting names that is refused: no project is written from it, a start
//! over it is refused naming it, and a run that starts nothing still goes ahead.

use lemonfiber_manifest::Manifest;

use super::machine;
use crate::app::Ctx;
use crate::error::codes::storage::ROOT_REFUSED;

/// An NFS export whose server is no host name.
const REFUSED: &str = "nfs://nas local/volume1";

/// The manifest of the stack this build carries.
fn manifest() -> Option<Manifest> {
    Manifest::from_toml(lemonfiber_bundled::STACK).ok()
}

/// A plan starting nothing in particular.
fn plan() -> crate::stack::closure::Plan {
    crate::stack::closure::Plan {
        forms: Vec::new(),
        profiles: std::collections::BTreeSet::new(),
        services: Vec::new(),
        dropped: Vec::new(),
        filtered: Vec::new(),
        footprint: crate::stack::closure::Footprint::default(),
        running: crate::stack::closure::Running::Unasked,
    }
}

/// What preparing the project for a run, starting where `starting`, came to.
fn preparing(ctx: &Ctx, starting: bool) -> Result<(), String> {
    manifest()
        .map(|manifest| super::super::prepared(ctx, &manifest, &plan(), starting))
        .map_or(Err(String::new()), |prepared| {
            prepared
                .map(|_| ())
                .map_err(|problem| problem.code.to_string())
        })
}

/// Write `value` as the machine's data root, by hand.
fn rooted_at(ctx: &Ctx, value: &str) {
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    assert!(crate::config::store::set(&env, crate::config::DATA_ROOT_KEY, value).is_ok());
}

#[test]
fn a_start_over_a_refused_data_root_is_refused_naming_it_and_writes_nothing() {
    let (ctx, project) = machine("rooted-start");
    rooted_at(&ctx, REFUSED);

    assert_eq!(preparing(&ctx, true), Err(ROOT_REFUSED.to_string()));
    assert!(!project.join(crate::project::FILE).exists());
}

#[test]
fn a_run_starting_nothing_over_a_refused_data_root_leaves_the_project_as_it_stands() {
    let (ctx, project) = machine("rooted-stop");
    rooted_at(&ctx, "/srv/media");
    assert_eq!(preparing(&ctx, false), Ok(()));
    let written = std::fs::read_to_string(project.join(crate::project::FILE)).ok();
    assert!(written
        .as_deref()
        .is_some_and(|text| text.contains("/srv/media")));
    rooted_at(&ctx, REFUSED);

    assert_eq!(preparing(&ctx, false), Ok(()));
    assert_eq!(
        std::fs::read_to_string(project.join(crate::project::FILE)).ok(),
        written
    );
}

#[test]
fn a_rewrite_over_a_refused_data_root_is_refused_naming_it() {
    let (ctx, project) = machine("rooted-rewrite");
    rooted_at(&ctx, REFUSED);

    let rewritten = manifest().map(|manifest| {
        super::super::rewritten(&ctx, &manifest, &[], "plugin tools", "stamp")
            .map_err(|problem| (problem.code.to_string(), problem.summary.clone()))
    });

    assert!(
        matches!(&rewritten, Some(Err((code, summary))) if *code == ROOT_REFUSED.to_string() && summary.contains(REFUSED)),
        "{rewritten:?}"
    );
    assert!(!project.join(crate::project::FILE).exists());
}
