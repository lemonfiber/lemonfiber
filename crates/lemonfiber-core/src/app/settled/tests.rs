use std::path::{Path, PathBuf};

use super::settled;
use crate::config::{store, Protocols, Settings, TORRENT_KEY, USENET_KEY};
use crate::model::{SetUp, UserFrom};
use crate::platform::Environment;
use crate::test_support::a_context;
use crate::wizard::{Phase, Wizard};

fn machine(name: &str) -> (crate::app::Ctx, PathBuf) {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("settled-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let config = dir.join("config");
    let stack = dir.join("data").join("stack");
    assert!(std::fs::create_dir_all(&config).is_ok());
    assert!(std::fs::create_dir_all(&stack).is_ok());
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(config.join(".env")),
            stack_dir: Some(stack),
            ..Settings::default()
        })
        .build();
    (ctx, config)
}

fn configured(config: &Path, text: &str) {
    assert!(store::write(&config.join(".env"), text).is_ok());
}

fn begun(ctx: &crate::app::Ctx, phase: Phase) {
    let mut progress = Wizard::new(Environment::LinuxNative).progress().clone();
    progress.phase = phase;
    let written = serde_json::to_string(&progress).unwrap_or_default();
    let at = crate::app::targets::layout(ctx).map(|paths| paths.setup_progress());
    assert!(at.is_some_and(|at| {
        at.parent()
            .is_some_and(|dir| std::fs::create_dir_all(dir).is_ok())
            && std::fs::write(at, written).is_ok()
    }));
}

#[test]
fn a_machine_with_nothing_configured_and_no_setup_begun_is_not_set_up() {
    let (ctx, _) = machine("none");

    let read = settled(&ctx);

    let Ok(read) = read else {
        unreachable!("the read was refused: {read:?}");
    };
    assert_eq!(read.setup, SetUp::None);
    assert_eq!(read.data_root, None);
    assert_eq!(read.protocols, Protocols::none());
    assert_eq!(read.service_user.from, UserFrom::StackOwner);
}

#[test]
fn a_configured_machine_says_what_setup_settled() {
    let (ctx, config) = machine("done");
    configured(
        &config,
        &format!(
            "DATA_ROOT=/srv/media\n{USENET_KEY}=true\n{TORRENT_KEY}=false\nPUID=1234\nPGID=4321\n"
        ),
    );

    let read = settled(&ctx);

    let Ok(read) = read else {
        unreachable!("the read was refused: {read:?}");
    };
    assert_eq!(read.setup, SetUp::Done);
    assert_eq!(read.data_root.as_deref(), Some("/srv/media"));
    assert!(read.protocols.usenet);
    assert!(!read.protocols.torrent);
    assert_eq!(
        (
            read.service_user.uid,
            read.service_user.gid,
            read.service_user.from
        ),
        (1234, 4321, UserFrom::Set)
    );
}

#[test]
fn an_nfs_data_root_is_said_as_it_is_written() {
    let (ctx, config) = machine("nfs");
    configured(&config, "DATA_ROOT=nfs://nas.local/volume1/media\n");

    let read = settled(&ctx).map(|read| read.data_root);

    assert_eq!(
        read.ok().flatten().as_deref(),
        Some("nfs://nas.local/volume1/media")
    );
}

#[test]
fn an_unset_service_user_is_the_owner_of_the_stack_directory() {
    let (ctx, config) = machine("owner");
    configured(&config, "DATA_ROOT=/srv/media\n");
    let owner = crate::app::project::directory(&ctx)
        .as_deref()
        .and_then(crate::plugin::adapting::running_as);

    let read = settled(&ctx).map(|read| read.service_user);

    let Ok(user) = read else {
        unreachable!("the read was refused: {read:?}");
    };
    assert_eq!(user.from, UserFrom::StackOwner);
    assert_eq!(
        Some((user.uid, user.gid)),
        owner.or(Some((
            crate::project::entry::OPERATOR_FALLBACK,
            crate::project::entry::OPERATOR_FALLBACK
        )))
    );
}

#[test]
fn one_id_set_is_said_as_part_set_with_the_other_the_owners() {
    let (ctx, config) = machine("part-set");
    configured(&config, "PUID=1234\n");

    let read = settled(&ctx).map(|read| read.service_user);

    let Ok(user) = read else {
        unreachable!("the read was refused: {read:?}");
    };
    assert_eq!((user.uid, user.from), (1234, UserFrom::PartSet));
}

#[test]
fn a_setup_begun_or_stopped_part_way_is_part_way_even_over_a_written_file() {
    for (name, phase, written) in [
        ("gathering", Phase::InProgress, false),
        ("reviewing", Phase::Reviewing, false),
        ("stopped", Phase::Applying, true),
    ] {
        let (ctx, config) = machine(name);
        if written {
            configured(&config, "DATA_ROOT=/srv/media\n");
        }
        begun(&ctx, phase);

        let read = settled(&ctx).map(|read| read.setup);

        assert_eq!(read.ok(), Some(SetUp::PartWay), "{name}");
    }
}

#[test]
fn a_finished_setup_on_a_written_file_is_done() {
    let (ctx, config) = machine("applied");
    configured(&config, "DATA_ROOT=/srv/media\n");
    begun(&ctx, Phase::Applied);

    let read = settled(&ctx).map(|read| read.setup);

    assert_eq!(read.ok(), Some(SetUp::Done));
}

#[test]
fn nowhere_to_keep_configuration_is_refused() {
    let ctx = a_context().settings(Settings::default()).build();

    assert!(settled(&ctx).is_err());
}
