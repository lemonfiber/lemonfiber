use super::{parse, WHOLE};

/// The line every field below is set beside, inside the plugin's one service.
const BESIDE: &str = "config_path = \"/config\"\n";

/// Networks are derived, never declared: a plugin service naming a network, a subnet or
/// an address is unparsable, and so is a `network` saying more than whose it runs inside.
#[test]
fn a_service_naming_a_network_a_subnet_or_an_address_is_refused() {
    let with = |field: &str| parse(&WHOLE.replacen(BESIDE, &format!("{BESIDE}{field}\n"), 1));
    for field in [
        r#"networks = ["media"]"#,
        r#"subnet = "10.80.0.0/24""#,
        r#"ipv4_address = "10.80.0.5""#,
        r#"ip_range = "10.80.0.128/25""#,
        r#"network = { via = "network.egress-guard", subnet = "10.80.0.0/24" }"#,
    ] {
        assert!(with(field).is_none(), "{field}");
    }
    assert!(with(r#"network = { via = "network.egress-guard" }"#).is_some());
}
