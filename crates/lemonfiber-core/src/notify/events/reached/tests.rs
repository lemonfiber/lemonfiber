//! Where a service is told to send what happened, and the certificate it pins there.

use super::{address, Told, HOST, PATH};
use crate::app::Ctx;
use crate::companion::served::{record, Served};
use crate::config::Settings;
use crate::platform::Environment;

/// The port the web surface was served on in these cases.
const PORT: u16 = 8471;

/// A machine on `environment`, keeping its configuration under a directory of its own,
/// with the operator's address where `written` names one.
fn machine(named: &str, environment: Environment, written: Option<&str>) -> Ctx {
    let dir =
        lemonfiber_fixtures::scratch::Scratch::named(&format!("events-reached-{named}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    crate::test_support::a_context()
        .environment(environment)
        .settings(Settings {
            companion: Some(dir),
            events_address: written.map(str::to_owned),
            ..Settings::default()
        })
        .build()
}

/// `ctx` with the web surface served on [`PORT`] as `encrypted` and `network` say.
fn served(ctx: Ctx, encrypted: bool, network: bool) -> Ctx {
    if let Some(dir) = ctx.settings.companion.as_deref() {
        let _ = record(
            dir,
            Served {
                port: PORT,
                encrypted,
                network,
            },
        );
    }
    ctx
}

/// The digest of the certificate `ctx` keeps.
fn pinned(ctx: &Ctx) -> String {
    ctx.settings
        .companion
        .as_deref()
        .and_then(|dir| crate::certificate::kept(ctx.seams.confined.as_ref(), dir).ok())
        .flatten()
        .map(|kept| kept.fingerprint)
        .unwrap_or_default()
}

#[test]
fn a_surface_served_encrypted_on_the_network_is_reached_by_its_host_name_and_pinned() {
    let ctx = served(
        machine("network", Environment::LinuxNative, None),
        true,
        true,
    );

    let told = address(&ctx);

    assert_eq!(
        told,
        Ok(Told {
            address: format!("https://{HOST}:{PORT}{PATH}"),
            pin: pinned(&ctx),
        })
    );
    assert_eq!(told.map(|told| told.pin.len()), Ok(64));
}

#[test]
fn a_surface_served_on_loopback_alone_is_reached_only_where_the_engine_forwards_there() {
    let forwarded = served(machine("forwarded", Environment::MacOs, None), true, false);
    let unforwarded = served(
        machine("unforwarded", Environment::LinuxNative, None),
        true,
        false,
    );

    assert!(address(&forwarded).is_ok());
    assert!(address(&unforwarded).is_err_and(|why| why.contains("only on the network")));
}

#[test]
fn a_surface_never_served_or_served_in_the_clear_is_reached_by_no_service() {
    let never = machine("never", Environment::MacOs, None);
    let clear = served(machine("clear", Environment::MacOs, None), false, true);

    assert!(address(&never).is_err_and(|why| why.contains("LEMONFIBER_EVENTS_ADDRESS")));
    assert!(address(&clear).is_err());
}

#[test]
fn the_address_the_operator_wrote_takes_the_place_of_the_host_name_with_the_same_pin() {
    let ctx = machine(
        "written",
        Environment::LinuxNative,
        Some("https://192.168.1.20:8471/"),
    );

    let told = address(&ctx);

    assert_eq!(
        told,
        Ok(Told {
            address: format!("https://192.168.1.20:8471{PATH}"),
            pin: pinned(&ctx),
        })
    );
}

#[test]
fn an_address_that_is_not_the_web_interfaces_https_one_is_refused_naming_the_setting() {
    for written in [
        "http://192.168.1.20:8471",
        "https://192.168.1.20:8471/api",
        "https://user@192.168.1.20:8471",
        "https://192.168.1.20:8471/?to=elsewhere",
        "not an address",
    ] {
        let ctx = machine("refused", Environment::LinuxNative, Some(written));
        assert!(
            address(&ctx).is_err_and(|why| why.contains("LEMONFIBER_EVENTS_ADDRESS")),
            "{written}"
        );
    }
}

#[test]
fn with_nowhere_to_keep_a_certificate_no_service_is_told_anywhere() {
    let ctx = crate::test_support::a_context().build();

    assert!(address(&ctx).is_err_and(|why| why.contains("configuration directory")));
}
