use super::*;

/// A plugin's container, as the engine reports one the stack does not declare.
fn stranger(id: &str, state: crate::docker::State) -> crate::docker::Undeclared {
    crate::docker::Undeclared {
        id: id.to_owned(),
        state,
        describes: crate::docker::UNDESCRIBED.to_owned(),
    }
}

/// The door over a stack holding a library and its proxy, and a plugin's request
/// service published through that proxy, with the plugin's container `state` where it
/// runs, the proxy `proxying` where it is up, and the proxy's domain `domain`.
fn over_a_plugin_door(
    state: Option<crate::docker::State>,
    proxying: bool,
    domain: Option<&str>,
) -> crate::model::FrontDoorReport {
    let mut proxy = service("front", Some(lemonfiber_manifest::Bind::Lan));
    proxy.provides = vec!["proxy.front".to_owned()];
    let declared = [watching(), proxy];
    let mut surveyed = vec![up("door", crate::docker::State::Healthy)];
    if proxying {
        surveyed.push(up("front", crate::docker::State::Running));
    }
    let plugin = crate::door::fixtures::fronted("requests", Some("request.intake@1"), Some("ask"));
    let undeclared: Vec<crate::docker::Undeclared> = state
        .map(|state| stranger("requests", state))
        .into_iter()
        .collect();
    assembled(
        &crate::door::candidates(
            &declared,
            std::slice::from_ref(&plugin),
            &crate::wiring::Fillers::default(),
        ),
        super::super::Running {
            surveyed: &surveyed,
            undeclared: &undeclared,
        },
        &Place {
            named: Some("kitchen-nas"),
            recorded: None,
            domain,
            environment: Environment::MacOs,
        },
        None,
        Whence::Home,
    )
}

/// A plugin's request service is the front door, handed over at the address the proxy
/// publishes it at, and the stack's library stands beside it at this machine's.
#[test]
fn a_plugins_request_service_is_the_door_at_the_address_the_proxy_publishes() {
    let report = over_a_plugin_door(Some(crate::docker::State::Running), true, Some("home.lan"));

    assert_eq!(report.standing, Standing::Established);
    assert_eq!(report.service.as_deref(), Some("requests the plugin's"));
    assert_eq!(
        report.address.map(|address| address.url),
        Some("https://ask.home.lan".to_owned())
    );
    assert!(report.beside.iter().any(|beside| beside.service == "door"
        && beside
            .address
            .as_ref()
            .is_some_and(|address| address.url.contains("kitchen-nas"))));
}

/// With no domain configured for the proxy, a plugin's door is answering and has no
/// address to hand anybody, and what is said names the setting that gives it one rather
/// than the household address a stack's door would need.
#[test]
fn a_plugins_door_with_no_domain_has_no_address_and_says_which_setting_gives_one() {
    for domain in [None, Some("home.example")] {
        let report = over_a_plugin_door(Some(crate::docker::State::Healthy), true, domain);

        assert_eq!(report.standing, Standing::Stranded, "{domain:?}");
        assert_eq!(report.address, None, "{domain:?}");
        assert!(
            report.meaning.contains(super::super::UNPROXIED.trim()),
            "{}",
            report.meaning
        );
        assert!(
            !report.meaning.contains("settings set lan-host"),
            "{}",
            report.meaning
        );
    }
}

/// A plugin's door whose container is not running is a door that is down, and so is
/// one whose container runs behind a proxy that does not: the household reaches it
/// through nothing else.
#[test]
fn a_plugins_door_that_is_not_running_or_not_proxied_is_unreachable() {
    for (state, proxying) in [
        (None, true),
        (Some(crate::docker::State::Starting), true),
        (Some(crate::docker::State::Running), false),
    ] {
        assert_eq!(
            over_a_plugin_door(state, proxying, Some("home.lan")).standing,
            Standing::Unreachable,
            "{state:?}, proxying: {proxying}"
        );
    }
}

/// A record of what is installed that will not read leaves the stack's own services to
/// answer for the door, rather than taking the door away with it.
#[tokio::test]
async fn an_unreadable_register_leaves_the_stacks_own_door() {
    let env = crate::test_support::env_without_password("door-unread");
    let beside = env.with_file_name(crate::config::paths::PLUGINS);
    assert!(std::fs::write(&beside, "not a register").is_ok());
    let watching = a_context()
        .engine(std::sync::Arc::new(Reporting::holding(
            &["door"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .settings(crate::config::Settings {
            env_file: Some(env),
            ..crate::config::Settings::default()
        })
        .build()
        .with_site(Renamed::called(Some("kitchen-nas")));

    let report = front_door(&watching, crate::app::Doorway::default())
        .await
        .ok();

    assert_eq!(
        report.as_ref().and_then(|report| report.service.clone()),
        Some("Jellyfin".to_owned())
    );
    assert_eq!(
        report.as_ref().map(|report| report.standing),
        Some(Standing::LibraryOnly)
    );
}
