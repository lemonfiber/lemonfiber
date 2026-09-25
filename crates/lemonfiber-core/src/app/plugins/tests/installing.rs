//! Installing a plugin: what is refused, and what is recorded.

use super::*;

/// The whole of the slice: what the manifest declared survives, and a later run
/// reads it back without the manifest being anywhere near.
#[tokio::test]
async fn where_a_service_keeps_its_configuration_survives_the_install_and_is_read_back() {
    let ctx = ctx("survives");
    let at = source("survives", MANIFEST);
    assert_eq!(counted(installing(&ctx, &at).await), Some(1));

    // The author's file goes, as it may the moment an install is done.
    let _ = std::fs::remove_dir_all(&at);

    let path = report(reading(&ctx).await)
        .and_then(|report| report.installed.first().cloned())
        .and_then(|one| one.services.first().cloned())
        .map(|one| one.config_path);
    assert_eq!(path.as_deref(), Some("/app/data"));
}

#[tokio::test]
async fn a_machine_with_nothing_installed_answers_with_an_empty_list() {
    let read = report(reading(&ctx("empty")).await);
    assert_eq!(read.as_ref().map(|one| one.installed.len()), Some(0));
    assert_eq!(read.map(|one| one.install.is_none()), Some(true));
}

#[tokio::test]
async fn the_install_says_what_it_recorded_and_what_it_joined() {
    let ctx = ctx("recorded");
    let shown = report(installing(&ctx, &source("recorded", MANIFEST)).await);
    let install = shown.as_ref().and_then(|one| one.install.clone());
    assert_eq!(install.as_ref().map(|one| one.recorded), Some(true));
    assert_eq!(
        install.map(|one| one.would.plugin),
        Some("komga".to_owned())
    );
    assert_eq!(shown.map(|one| one.installed.len()), Some(1));
}

/// The gate a rehearsal exists to pass: the account is the same and the file is
/// not there afterwards.
#[tokio::test]
async fn a_rehearsed_install_says_everything_the_real_one_would_and_writes_nothing() {
    let ctx = rehearsing("rehearsed");
    let install = report(installing(&ctx, &source("rehearsed", MANIFEST)).await)
        .and_then(|one| one.install.clone());
    assert_eq!(
        install.as_ref().map(|one| one.would.plugin.clone()),
        Some("komga".to_owned())
    );
    assert_eq!(install.map(|one| one.recorded), Some(false));
    assert!(!record_of(&ctx).exists(), "the record was written");
    assert_eq!(counted(reading(&ctx).await), Some(0));
}

/// And the listing beside it counts what is installed rather than what would
/// be. A rehearsal that said *one plugin is installed* in the same breath as
/// *nothing was written* is a rehearsal an operator has to choose between two
/// halves of.
#[tokio::test]
async fn a_rehearsed_install_is_not_counted_among_what_is_installed() {
    let ctx = rehearsing("uncounted");
    assert_eq!(
        counted(installing(&ctx, &source("uncounted", MANIFEST)).await),
        Some(0)
    );
}

#[tokio::test]
async fn a_path_holding_no_manifest_is_refused_rather_than_installed() {
    let ctx = ctx("nothing-there");
    assert_eq!(
        refusal(installing(&ctx, Path::new("/nowhere/at/all")).await),
        "PLUGIN-2"
    );
    assert!(!record_of(&ctx).exists());
}

/// The reader's verdict is total, and the install honours it: a manifest that
/// over-reaches on where it keeps its state is not installed at all.
#[tokio::test]
async fn a_manifest_this_build_refuses_is_not_installed() {
    let ctx = ctx("refused");
    let at = source(
        "refused",
        &MANIFEST.replace(
            r#"config_path = "/app/data""#,
            r#"config_path = "/data/media""#,
        ),
    );
    assert_eq!(refusal(installing(&ctx, &at).await), "PLUGIN-3");
    assert!(!record_of(&ctx).exists(), "the record was written");
}

/// A plugin needing something of lemonfiber that this build does not offer is
/// refused by naming the thing, never a version. *Too old* is the wrong sentence for
/// a mechanism that went, and an operator cannot upgrade their way to one that was
/// never there — so the refusal says which one, and says what this build does
/// offer, and nothing is written.
#[tokio::test]
async fn a_plugin_needing_what_this_build_does_not_offer_is_refused_by_name() {
    let ctx = ctx("unoffered");
    let at = source(
        "unoffered",
        &format!("{MANIFEST}\n[requires]\ncapabilities = [\"service.add\"]\n"),
    );

    let detail = installing(&ctx, &at)
        .await
        .err()
        .map(|problem| (problem.code.to_string(), problem.detail.unwrap_or_default()))
        .unwrap_or_default();

    assert_eq!(detail.0, "PLUGIN-3");
    assert!(
        detail
            .1
            .contains("service.add is not something this build offers a plugin"),
        "the capability is named: {}",
        detail.1
    );
    assert!(
        detail.1.contains("doctor.contribute"),
        "and so is what this build does offer: {}",
        detail.1
    );
    assert!(
        !detail.1.contains("version"),
        "and no version is named, because none is the reason: {}",
        detail.1
    );
    assert!(!record_of(&ctx).exists(), "nothing was installed");
    assert!(made_paths(&ctx).is_empty(), "and nothing was written");
}

#[tokio::test]
async fn installing_what_is_installed_is_refused_naming_it() {
    let ctx = ctx("twice");
    let at = source("twice", MANIFEST);
    assert_eq!(counted(installing(&ctx, &at).await), Some(1));
    assert_eq!(refusal(installing(&ctx, &at).await), "PLUGIN-5");
    assert_eq!(counted(reading(&ctx).await), Some(1));
}

/// The gate this record exists to pass, in the place it runs. A damaged record
/// read as empty would answer *nothing is installed* about a machine running
/// somebody else's service — and then install a second copy over it.
#[tokio::test]
async fn a_damaged_record_refuses_the_read_and_the_install_rather_than_reading_as_empty() {
    let ctx = ctx("damaged");
    let at = source("damaged", MANIFEST);
    assert_eq!(counted(installing(&ctx, &at).await), Some(1));
    assert!(crate::config::store::write(&record_of(&ctx), "{ half a record").is_ok());

    assert_eq!(refusal(reading(&ctx).await), "PLUGIN-4");
    assert_eq!(refusal(installing(&ctx, &at).await), "PLUGIN-4");
    // And a refusal is not an answer with a shorter listing in it: there is no
    // report at all, which is what stops a surface rendering one.
    assert_eq!(report(reading(&ctx).await), None);
}

/// A record that is there and cannot be opened at all is the same answer as one
/// that will not parse, and for the same reason: the one thing that must not
/// happen is answering *nothing is installed*.
#[tokio::test]
async fn a_record_that_cannot_be_opened_is_refused_rather_than_read_as_empty() {
    let ctx = ctx("unopenable");
    assert!(std::fs::create_dir_all(record_of(&ctx)).is_ok());
    assert_eq!(refusal(reading(&ctx).await), "PLUGIN-4");
}

/// Nowhere configured is a machine that has not been set up, which has no
/// plugins rather than an unreadable record.
#[tokio::test]
async fn a_machine_with_nowhere_to_keep_a_record_reads_as_nothing_installed() {
    assert_eq!(counted(reading(&a_context().build()).await), Some(0));
}

/// And refuses to write one, because the alternative is telling an operator
/// something was remembered that was not.
#[tokio::test]
async fn a_machine_with_nowhere_to_keep_a_record_refuses_to_install() {
    let ctx = a_context().build();
    assert!(!refusal(installing(&ctx, &source("nowhere", MANIFEST)).await).is_empty());
}

/// Installing a plugin whose service claims what the stack asks for says, before it
/// happens, that the ask will be contested and reach nothing until somebody chooses —
/// read against the stack this build carries.
#[tokio::test]
async fn an_install_says_which_asks_it_would_leave_contested() {
    let ctx = ctx("contesting");
    let would = crate::plugin::read(&source("contesting", MANIFEST))
        .ok()
        .map(|manifest| crate::plugin::Installed::of(&manifest))
        .map(|mut would| {
            let _ = would.services.first_mut().map(|placed| {
                placed.provides = vec!["identity.source".to_owned()];
            });
            would
        });

    let contests = would
        .and_then(|would| {
            super::super::standing::contested(&ctx, &crate::plugin::Register::empty(), &would).ok()
        })
        .unwrap_or_default();

    assert_eq!(contests.len(), 1, "{contests:?}");
    assert!(contests.first().is_some_and(|one| one.by == "seerr"
        && one.capability == "identity.source"
        && one
            .claimants
            .iter()
            .any(|named| named == "komga (plugin komga)")));
}

/// An install or an update on a stack this build cannot read is refused before
/// anything is written, because what it would do to the wiring cannot be stated.
#[tokio::test]
async fn an_install_or_update_on_an_unreadable_stack_is_refused_first() {
    let ctx = proving(
        "contest-blind",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    assert_eq!(
        counted(installing(&ctx, &source("contest-blind", PROVING)).await),
        Some(1)
    );
    let blind = a_context()
        .over(crate::test_support::nowhere())
        .settings(ctx.settings.clone())
        .build();
    let refused = crate::error::codes::stack::STACK_UNREADABLE.to_string();
    assert_eq!(
        refusal(updating(&blind, &source("contest-blind-next", &next())).await),
        refused
    );

    let fresh = a_context()
        .over(crate::test_support::nowhere())
        .settings(crate::config::Settings {
            env_file: Some(env_at("contest-blind-fresh", &a_password())),
            stack_dir: ctx.settings.stack_dir.clone(),
            ..crate::config::Settings::default()
        })
        .build();
    assert_eq!(
        refusal(installing(&fresh, &source("contest-blind", PROVING)).await),
        refused
    );
}

/// The one read of what each plugin is doing: where it came from and when, that
/// nobody reviewed it, what it declared, and what the operator chose it to stand in
/// for — all from the record, with the author's directory gone.
#[tokio::test]
async fn the_reading_says_where_a_plugin_came_from_when_and_what_it_stands_in_for() {
    let ctx = proving(
        "one-read",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    let at = source("one-read", PROVING);
    assert_eq!(counted(installing(&ctx, &at).await), Some(1));
    let _ = std::fs::remove_dir_all(&at);
    let _ = ctx.settings.env_file.as_deref().map(|file| {
        crate::config::store::set(file, crate::wiring::FILLS_KEY, "media.serve=komga,x=y")
    });

    let read = report(reading(&ctx).await);
    let one = read.as_ref().and_then(|one| one.installed.first());

    assert_eq!(
        one.map(|one| one.from.as_str()),
        Some(at.display().to_string().as_str())
    );
    assert_eq!(
        one.map(|one| one.installed_at.clone()),
        Some(ctx.stamp()),
        "stamped as its changes are journalled"
    );
    assert_eq!(one.map(|one| one.declared.reviewed), Some(false));
    assert_eq!(
        read.map(|one| one.substituted),
        Some(vec![crate::plugin::Substituted {
            plugin: "komga".to_owned(),
            capability: "media.serve".to_owned(),
            service: "komga".to_owned(),
        }]),
        "a choice naming a service no plugin brought is not this read's"
    );
}
