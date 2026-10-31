use super::super::super::register::Register;
use super::installed;

/// A record an older lemonfiber wrote, naming an adapter for a plugin, for one of its
/// services and for one of its recipe steps, still reads, and what is written again names
/// none.
#[test]
fn a_record_naming_adapters_reads_and_is_written_without_them() {
    let mut register = Register::empty();
    assert_eq!(
        installed(|_| {}).map(|one| register.record(one)),
        Some(Ok(()))
    );
    let mut older: serde_json::Value =
        serde_json::from_str(&register.to_json().unwrap_or_default()).unwrap_or_default();
    if let Some(plugin) = older
        .get_mut("installed")
        .and_then(|installed| installed.get_mut(0))
        .and_then(serde_json::Value::as_object_mut)
    {
        plugin.insert(
            "adapters".to_owned(),
            serde_json::json!([{ "service": "komga", "kind": "sabnzbd", "owner": "lemonfiber" }]),
        );
        plugin.insert(
            "recipes".to_owned(),
            serde_json::json!([{
                "id": "adopt", "title": "Adopt", "why": "Why", "on": "install",
                "inputs": [], "pairs": [],
                "steps": [{
                    "id": "ask", "method": "GET", "to": "sonarr", "path": "/api/v3/series",
                    "adapter": { "kind": "servarr", "owner": "lemonfiber" }
                }]
            }]),
        );
        if let Some(service) = plugin
            .get_mut("services")
            .and_then(|services| services.get_mut(0))
            .and_then(serde_json::Value::as_object_mut)
        {
            service.insert(
                "api".to_owned(),
                serde_json::json!({ "kind": "sabnzbd", "key_source": "config-ini" }),
            );
        }
    }
    let text = older.to_string();
    assert!(
        text.contains("\"adapters\"") && text.contains("\"api\"") && text.contains("\"adapter\""),
        "{text}"
    );

    let read = Register::parse(&text);
    let again = read
        .as_ref()
        .ok()
        .and_then(Register::to_json)
        .unwrap_or_default();

    assert!(read.is_ok(), "{read:?}");
    assert!(!again.contains("\"adapters\""), "{again}");
    assert!(!again.contains("\"api\""), "{again}");
    assert!(!again.contains("\"adapter\""), "{again}");
    assert!(again.contains("\"/api/v3/series\""), "{again}");
}
