use bollard::models::MountType;

use super::{bounded, reaching, FROM};

#[test]
fn the_container_reaching_a_volume_mounts_it_read_only_and_reaches_no_network() {
    let body = reaching("lemonfiber_caddy_data", "ghcr.io/lemonfiber/caddy:1");

    let host = body.host_config.unwrap_or_default();
    let mounts = host.mounts.unwrap_or_default();
    assert_eq!(body.image.as_deref(), Some("ghcr.io/lemonfiber/caddy:1"));
    assert_eq!(body.network_disabled, Some(true));
    assert_eq!(host.network_mode.as_deref(), Some("none"));
    assert!(matches!(mounts.as_slice(), [one]
        if one.typ == Some(MountType::VOLUME)
            && one.source.as_deref() == Some("lemonfiber_caddy_data")
            && one.target.as_deref() == Some(FROM)
            && one.read_only == Some(true)));
}

#[test]
fn an_archive_larger_than_is_copied_is_refused_and_one_at_the_bound_is_not() {
    assert!(bounded(10, 10).is_ok());
    assert!(bounded(11, 10).is_err());
}
