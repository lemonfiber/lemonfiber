//! The core's own settings, recorded, read back and told of through a change.

use super::changing::{agreed, machine, recorded, running, Machine};

#[tokio::test]
async fn a_core_setting_is_recorded_in_the_store_and_read_back_as_the_cores() {
    let machine = machine("core", running(&[]));
    let identities = format!(
        "android:app.example.lemon@{},ios:ABCDE12345.app.example.lemon",
        "a1".repeat(32)
    );

    let made = agreed(
        &machine.ctx,
        crate::settings::own::DEVICE_KEY_APPS,
        Some(&identities),
    )
    .await;

    let read = made.ok().and_then(|report| {
        report
            .settings
            .into_iter()
            .find(|one| one.name == crate::settings::own::DEVICE_KEY_APPS)
    });
    assert!(
        read.is_some_and(|one| one.whose == crate::model::Owner::Core
            && one.readers.is_empty()
            && one.value.as_deref() == Some(identities.as_str()))
    );
    assert_eq!(
        recorded(&machine, crate::settings::own::DEVICE_KEY_APPS),
        Some(identities)
    );
}

#[tokio::test]
async fn every_core_setting_is_read_with_what_the_core_reads_while_none_is_recorded() {
    use crate::settings::own::{refusal, CORE, DEVICE_KEY_APPS, GUIDE_HOURS};
    let machine = machine("core-defaults", running(&[]));
    let read = |machine: &Machine| {
        crate::settings::run::listing(&machine.ctx)
            .map(|report| report.settings)
            .unwrap_or_default()
    };

    let unset = read(&machine);
    for (name, declared) in CORE {
        let default = declared.written();
        assert!(default.is_some() || name == DEVICE_KEY_APPS, "{name}");
        assert_eq!(
            unset
                .iter()
                .find(|one| one.name == name)
                .map(|one| (one.value.clone(), one.default.clone())),
            Some((None, default.clone())),
            "{name}"
        );
        assert_eq!(
            default.and_then(|value| refusal(name, &value)),
            None,
            "{name}"
        );
    }

    assert!(agreed(&machine.ctx, GUIDE_HOURS, Some("24")).await.is_ok());
    assert_eq!(
        read(&machine)
            .into_iter()
            .find(|one| one.name == GUIDE_HOURS)
            .map(|one| (one.value, one.default)),
        Some((Some("24".to_owned()), Some("12".to_owned())))
    );
}

#[tokio::test]
async fn changing_the_device_key_apps_tells_the_operator_of_each_identity_gained_and_lost() {
    let machine = machine("device-key-apps", running(&[]));
    let identity = "ios:ABCDE12345.app.example.lemon";
    let set = agreed(
        &machine.ctx,
        crate::settings::own::DEVICE_KEY_APPS,
        Some(identity),
    )
    .await;
    assert!(set.is_ok());
    let cleared = agreed(&machine.ctx, crate::settings::own::DEVICE_KEY_APPS, None).await;
    assert!(cleared.is_ok());
    let told: Vec<String> = crate::app::outbox::load(&machine.ctx)
        .history()
        .into_iter()
        .map(|alert| alert.summary.clone())
        .collect();
    assert!(told.contains(&format!("The app {identity} may now register device keys")));
    assert!(told.contains(&format!(
        "The app {identity} may no longer register device keys"
    )));
}
