use crate::refusing::refusals;
use crate::schema::tests::WHOLE;
use crate::schema::Manifest;

/// The service the whole fixture opens with.
const KOMGA: &str = "[[service]]\nid          = \"komga\"";

/// A run-and-exit service beside the fixture's, declaring `extra` and run at `default`
/// as `setting` says.
fn scheduling(extra: &str, setting: &str, default: &str) -> Vec<String> {
    assert!(WHOLE.contains(KOMGA));
    let service = format!(
        "[[service]]\nid          = \"komga-sweep\"\nname        = \"Komga sweep\"\nimage       = \"docker.io/gotson/komga\"\ndigest      = \"sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945\"\ntag         = \"1.11.0\"\ncriticality = \"enhancing\"\nschedule    = {{ setting = \"{setting}\", default = \"{default}\" }}\n{extra}\n\n"
    );
    let text = WHOLE.replacen(KOMGA, &format!("{service}{KOMGA}"), 1);
    Manifest::from_toml(&text).map_or_else(
        |refused| vec![refused.to_string()],
        |manifest| {
            refusals(&manifest, &["storage.hardlinks"])
                .iter()
                .map(ToString::to_string)
                .filter(|said| said.contains("schedule"))
                .collect()
        },
    )
}

#[test]
fn a_service_nothing_reaches_runs_at_the_times_a_setting_of_its_own_names() {
    for default in ["15 3 * * *", "@daily", "*/30 * * * 1-5"] {
        let said = scheduling("", "komga:sweep-at", default);
        assert!(said.is_empty(), "{default}: {said:?}");
    }
}

#[test]
fn a_default_that_names_no_time_is_refused_naming_the_service() {
    for default in ["", "@reboot", "every night", "61 * * * *", "* * * *"] {
        let said = scheduling("", "komga:sweep-at", default);
        assert!(
            said.iter()
                .any(|one| one.contains("schedule.default") && one.contains("komga-sweep")),
            "{default:?}: {said:?}"
        );
    }
}

#[test]
fn a_setting_outside_the_plugins_namespace_is_refused() {
    for setting in ["sweep-at", "sonarr:sweep-at", "komga:"] {
        let said = scheduling("", setting, "@daily");
        assert!(
            said.iter().any(|one| one.contains("schedule.setting")),
            "{setting}: {said:?}"
        );
    }
}

#[test]
fn a_scheduled_service_something_reaches_is_refused_naming_why() {
    for (extra, why) in [
        (
            "speaks      = [\"komga:sweep@1\"]\nlistens     = 8080",
            "speaks a contract",
        ),
        ("port        = 25601", "publishes a port"),
        ("bind        = \"loopback\"", "declares bind"),
        ("provides    = [\"komga:swept\"]", "provides a capability"),
    ] {
        let said = scheduling(extra, "komga:sweep-at", "@daily");
        assert!(
            said.iter()
                .any(|one| one.contains("komga-sweep") && one.contains(why)),
            "{why}: {said:?}"
        );
    }
}
