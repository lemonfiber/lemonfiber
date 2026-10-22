use super::{broken, files, names, PART, TABLES};

/// An assembled part's one service carries the part's id, since it is the service that
/// takes the part's place in the stack.
#[test]
fn an_assembled_part_s_service_carries_the_part_s_id() {
    let renamed = PART.replace("id = \"sabnzbd\"\nprovides", "id = \"usenet\"\nprovides");
    let said = broken("assembled-renamed", &renamed, &files(), &[], TABLES);
    assert!(
        names(
            &said,
            &["plugin sabnzbd is assembled", "service.id is sabnzbd"]
        ),
        "{said:?}"
    );
}

/// An assembled part's claims keep their fixtures under its own id's recordings, and one
/// elsewhere in its tree is refused naming the fixture.
#[test]
fn an_assembled_part_s_fixture_outside_its_recordings_is_refused() {
    let elsewhere = PART.replace(
        "recordings/sabnzbd/queue.json",
        "recordings/other/queue.json",
    );
    let mut moved = files();
    moved.remove("recordings/sabnzbd/queue.json");
    moved.insert(
        "recordings/other/queue.json".to_owned(),
        b"{\"status\": 200}\n".to_vec(),
    );
    let said = broken("assembled-elsewhere", &elsewhere, &moved, &[], TABLES);
    assert!(
        names(
            &said,
            &[
                "plugin sabnzbd is assembled",
                "recordings/other/queue.json",
                "recordings/sabnzbd/"
            ]
        ),
        "{said:?}"
    );
}

/// Profiles, forms and removed services are the bundle's to declare, and a plugin
/// manifest declaring any of them is refused naming the table.
#[test]
fn a_plugin_manifest_declaring_a_stack_table_is_refused() {
    for table in ["profile", "form", "removed"] {
        let declaring = format!("{PART}\n[[{table}]]\nid = \"extra\"\n");
        let said = broken(
            &format!("declares-{table}"),
            &declaring,
            &files(),
            &[],
            TABLES,
        );
        assert!(
            names(
                &said,
                &[
                    "sabnzbd/plugin.toml",
                    &format!("[[{table}]]"),
                    "only the bundle"
                ]
            ),
            "{said:?}"
        );
    }
}

/// A pin leaving out a field every pin names is refused naming the plugin and the field,
/// and one naming no id is named by its place.
#[test]
fn a_pin_leaving_out_a_field_is_refused_naming_the_plugin_and_the_field() {
    let unoriginal = super::bundle(PART, &files(), &[], TABLES)
        .lines()
        .filter(|line| !line.starts_with("origin = "))
        .collect::<Vec<_>>()
        .join("\n");
    let root = super::catalogue("unoriginal", &unoriginal, "sabnzbd", PART, &files());
    assert_eq!(
        super::refused(&root),
        vec!["plugin sabnzbd.origin is missing, which every pin names".to_owned()]
    );
    let anonymous = super::bundle(PART, &files(), &[], TABLES)
        .lines()
        .filter(|line| !line.starts_with("id = \"sabnzbd\""))
        .collect::<Vec<_>>()
        .join("\n");
    let root = super::catalogue("anonymous", &anonymous, "sabnzbd", PART, &files());
    assert_eq!(
        super::refused(&root),
        vec!["[[plugin]] 1.id is missing, which every pin names".to_owned()]
    );
}
