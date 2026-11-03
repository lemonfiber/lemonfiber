use super::super::{Manifest, Service};
use super::WHOLE;

/// The container fields one TOML body declares, read as a stack service's and as a
/// plugin service's.
const CONTAINED: &str = r#"
takes_data  = true
config_path = "/config"
tmpfs       = ["/tmp", "/config"]
mount       = [{ from = "cache", to = "/cache" }, { from = "cache", to = "/again" }, { from = "../etc", to = "/data/etc" }]
user_mode   = "image"
posture     = "confined"
mem_limit_mib = 0
env         = { TZ = "Europe/Amsterdam", MODE = { setting = "elsewhere:mode", default = "fast" }, LEVEL = { setting = "komga:level", default = "info" } }
"#;

/// Every field the one container model declares, which the stack's services and every
/// plugin's take alike.
const CONTAINER_FIELDS: [&str; 15] = [
    "publish",
    "takes_data",
    "config_path",
    "config_ro",
    "mount",
    "shares_config",
    "rebuilt",
    "tmpfs",
    "env",
    "user_mode",
    "posture",
    "mem_limit_mib",
    "network",
    "shape",
    "certificate",
];

#[test]
fn a_stack_service_and_a_plugin_service_declare_one_set_of_container_fields_of_one_type() {
    let stack = serde_json::to_value(schemars::schema_for!(lemonfiber_manifest::Manifest))
        .unwrap_or_default();
    let plugin = serde_json::to_value(schemars::schema_for!(Manifest)).unwrap_or_default();
    let field = |schema: &serde_json::Value, service: &str, name: &str| {
        schema
            .pointer(&format!("/$defs/{service}/properties/{name}"))
            .cloned()
    };
    for name in CONTAINER_FIELDS {
        let declared = field(&stack, "Service", name);
        assert!(declared.is_some(), "the stack declares {name}");
        assert_eq!(declared, field(&plugin, "PluginService", name), "{name}");
    }
}

#[test]
fn a_stack_service_and_a_plugin_service_are_refused_alike_for_the_same_fields() {
    let stack: Result<lemonfiber_manifest::Service, _> = toml::from_str(&format!(
        r#"
id           = "komga"
name         = "Komga"
profile      = "library"
image        = "docker.io/gotson/komga"
tag          = "1.11.0"
criticality  = "important"
license      = "MIT"
upstream     = "https://github.com/gotson/komga"
last_release = "2026-01-01"
describes    = "Reads comics"
without_it   = "Files on disk, no way to read them"
{CONTAINED}"#
    ));
    let plugin: Result<Service, _> = toml::from_str(&format!(
        r#"
id     = "komga"
name   = "Komga"
image  = "docker.io/gotson/komga"
digest = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag    = "1.11.0"
criticality = "important"
{CONTAINED}"#
    ));
    let (Ok(stack), Ok(plugin)) = (&stack, &plugin) else {
        unreachable!("both read: {:?} {:?}", stack.err(), plugin.err());
    };
    let refused = stack.container().broken();
    assert_eq!(refused.len(), 9, "{refused:#?}");
    assert_eq!(plugin.container("komga", &[]).broken(), refused);
}

#[test]
fn a_variable_of_no_kind_the_model_knows_is_refused_naming_the_variable() {
    for (variable, value) in [
        ("LISTED", "[1, 2]"),
        ("TABLED", r#"{ from = "host" }"#),
        ("ENVIRONED", r#"{ env = "HOME" }"#),
    ] {
        let manifest = WHOLE.replace(
            "config_path = \"/config\"\n",
            &format!("config_path = \"/config\"\nenv = {{ {variable} = {value} }}\n"),
        );
        let refusal = Manifest::from_toml(&manifest)
            .err()
            .map(|refused| refused.to_string())
            .unwrap_or_default();
        assert!(refusal.contains(variable), "{variable}: {refusal}");
    }
}

#[test]
fn a_variable_lemonfiber_writes_itself_is_refused_naming_it() {
    for variable in lemonfiber_manifest::container::RESERVED {
        let manifest = WHOLE.replace(
            "config_path = \"/config\"\n",
            &format!("config_path = \"/config\"\nenv = {{ {variable} = \"1\" }}\n"),
        );
        let refusals: Vec<String> = Manifest::from_toml(&manifest)
            .map(|read| crate::refusing::refusals(&read, &[]))
            .unwrap_or_default()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert!(
            refusals
                .iter()
                .any(|refusal| refusal.contains(&format!("env.{variable}"))
                    && refusal.contains("lemonfiber's to write")),
            "{variable}: {refusals:#?}"
        );
    }
}
