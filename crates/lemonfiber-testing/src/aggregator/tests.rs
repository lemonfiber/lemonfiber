use lemonfiber_core::ports::filesystem::Beneath;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::scratch::Scratch;

use super::super::context::a_context;
use super::{asking, moved, running, ADAPTER, PLUGIN};

#[test]
fn the_moved_plugin_is_nzbhydra2s_with_an_adapter_claiming_and_searching_through_it() {
    let installed = moved();

    assert_eq!(
        installed.as_ref().map(|one| one.plugin.as_str()),
        Some(PLUGIN)
    );
    assert!(
        installed.is_some_and(|one| one.services.iter().any(|placed| {
            placed.service == ADAPTER
                && placed.fronts.as_deref() == Some(PLUGIN)
                && ["owner.claim@1", "indexer.search@1"]
                    .iter()
                    .all(|contract| placed.speaks.iter().any(|spoken| spoken == contract))
        }))
    );
}

#[test]
fn an_aggregator_asked_holds_its_key_wherever_its_stack_is_and_writes_nothing() {
    let directory = Scratch::new("aggregator-asking");
    let ctx = asking(
        a_context()
            .over(Source::External(Box::leak(
                directory.to_path_buf().into_boxed_path(),
            )))
            .build(),
    );

    let at = lemonfiber_core::plugin::key_file(&directory, ADAPTER);
    assert!(!at.exists());
    assert_eq!(
        ctx.seams.confined.read(&at, &directory),
        Beneath::Read(super::super::adapted::KEY.to_owned())
    );
}

#[test]
fn the_running_adapter_is_healthy_and_published_on_the_adapters_port() {
    assert!(super::super::adapted::runs_on_the_adapters_port(
        &running(),
        ADAPTER
    ));
}
