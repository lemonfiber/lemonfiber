use std::sync::Arc;

use lemonfiber_adapters::filesystem::Disk;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_ports::service::{Entry, EntryWidget, Group, PageWidget, Panels, Showing};

use super::{unmanaged, Configured, SERVICES, WIDGETS};
use crate::materialised::{checksum, Materialised};

/// Homepage's files as the stack ships them, before anything is applied.
const SHIPPED: &str = "- Watch:\n    - Jellyfin:\n        href: http://x:8096\n";

/// Homepage under a fresh stack directory holding its shipped files, and the record of
/// what lemonfiber materialised there.
fn homepage(name: &str) -> (Scratch, Configured) {
    let scratch = Scratch::new(name);
    let mut record = Materialised::new();
    for key in [SERVICES, WIDGETS] {
        let at = scratch.path().join(key);
        let _ = at.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(&at, SHIPPED);
        record.record(key, checksum(SHIPPED.as_bytes()));
    }
    let kept = scratch.path().join("materialised.json");
    let _ = std::fs::write(&kept, serde_json::to_string(&record).unwrap_or_default());
    let configured = Configured::at(scratch.path(), Arc::new(Disk), Some(kept));
    (scratch, configured)
}

/// A panel list with one entry of every shape.
fn panels() -> Panels {
    Panels {
        groups: vec![
            Group {
                name: "Watch".to_owned(),
                entries: vec![Entry {
                    name: "Jellyfin".to_owned(),
                    icon: "jellyfin.png".to_owned(),
                    link: "http://192.168.1.20:8096".to_owned(),
                    description: "Films and television: all of it".to_owned(),
                    check: Some("http://jellyfin:8096/health".to_owned()),
                    widget: None,
                }],
            },
            Group {
                name: "Automation".to_owned(),
                entries: vec![
                    Entry {
                        name: "Uptime".to_owned(),
                        icon: "mdi-heart-pulse".to_owned(),
                        link: "http://localhost:3001".to_owned(),
                        description: "Watches the rest".to_owned(),
                        check: None,
                        widget: Some(EntryWidget::StatusPage {
                            url: "http://uptime-kuma:3001".to_owned(),
                            page: "household".to_owned(),
                        }),
                    },
                    Entry {
                        name: "Sonarr".to_owned(),
                        icon: "sonarr.png".to_owned(),
                        link: "http://localhost:8989".to_owned(),
                        description: "Television".to_owned(),
                        check: None,
                        widget: None,
                    },
                ],
            },
        ],
        widgets: vec![
            PageWidget::Resources,
            PageWidget::Search {
                url: "https://duckduckgo.com/?q=".to_owned(),
            },
        ],
    }
}

/// What the file `key` holds, as JSON.
fn file(scratch: &Scratch, key: &str) -> serde_json::Value {
    std::fs::read_to_string(scratch.path().join(key))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

#[tokio::test]
async fn the_list_it_shows_is_the_list_applied() {
    let (_scratch, homepage) = homepage("homepage-applied");

    assert!(homepage.apply(&panels()).await.is_ok());

    assert_eq!(homepage.panels().await.ok(), Some(panels()));
}

#[tokio::test]
async fn the_files_are_written_in_the_shape_homepage_reads() {
    let (scratch, homepage) = homepage("homepage-shape");

    assert!(homepage.apply(&panels()).await.is_ok());

    assert_eq!(
        file(&scratch, SERVICES),
        serde_json::json!([
            {"Watch": [{"Jellyfin": {
                "icon": "jellyfin.png",
                "href": "http://192.168.1.20:8096",
                "description": "Films and television: all of it",
                "siteMonitor": "http://jellyfin:8096/health",
            }}]},
            {"Automation": [
                {"Uptime": {
                    "icon": "mdi-heart-pulse",
                    "href": "http://localhost:3001",
                    "description": "Watches the rest",
                    "widget": {"type": "uptimekuma", "url": "http://uptime-kuma:3001", "slug": "household"},
                }},
                {"Sonarr": {
                    "icon": "sonarr.png",
                    "href": "http://localhost:8989",
                    "description": "Television",
                }},
            ]},
        ])
    );
    assert_eq!(
        file(&scratch, WIDGETS),
        serde_json::json!([
            {"resources": {"cpu": true, "memory": true, "expanded": true}},
            {"search": {"provider": "custom", "url": "https://duckduckgo.com/?q=", "target": "_blank"}},
        ])
    );
}

#[tokio::test]
async fn the_record_of_what_was_materialised_moves_with_each_file() {
    let (scratch, homepage) = homepage("homepage-record");

    assert!(homepage.apply(&panels()).await.is_ok());

    let kept: Materialised =
        crate::app::record::kept(Some(&scratch.path().join("materialised.json")));
    for key in [SERVICES, WIDGETS] {
        let now = std::fs::read(scratch.path().join(key)).unwrap_or_default();
        assert_eq!(kept.checksum(key), Some(checksum(&now)), "{key}");
    }
}

#[tokio::test]
async fn files_it_did_not_write_show_nothing() {
    let (_scratch, homepage) = homepage("homepage-shipped");

    assert_eq!(
        homepage.panels().await.ok(),
        Some(Panels {
            groups: Vec::new(),
            widgets: Vec::new(),
        })
    );
}

#[tokio::test]
async fn a_panel_list_homepage_would_expand_is_refused_and_nothing_is_written() {
    let mut expanding = panels();
    if let Some(entry) = expanding
        .groups
        .first_mut()
        .and_then(|group| group.entries.first_mut())
    {
        entry.description = "{{HOMEPAGE_VAR_KEY}}".to_owned();
    }
    let mut searching = panels();
    searching.widgets = vec![PageWidget::Search {
        url: "https://x/?q={{HOMEPAGE_FILE_KEY}}".to_owned(),
    }];

    for (named, refused) in [("entry", expanding), ("search", searching)] {
        let (scratch, homepage) = homepage(&format!("homepage-expanding-{named}"));
        let said = homepage.apply(&refused).await;
        assert!(
            said.as_ref()
                .is_err_and(|failure| failure.to_string().contains("expand")),
            "{named}: {said:?}"
        );
        assert_eq!(
            std::fs::read_to_string(scratch.path().join(SERVICES))
                .ok()
                .as_deref(),
            Some(SHIPPED)
        );
    }
}

#[tokio::test]
async fn a_file_that_is_a_link_is_neither_read_nor_written_through() {
    for (named, key) in [("services", SERVICES), ("widgets", WIDGETS)] {
        let (scratch, homepage) = homepage(&format!("homepage-linked-{named}"));
        let elsewhere = scratch.path().join("elsewhere.yaml");
        let _ = std::fs::write(&elsewhere, SHIPPED);
        let at = scratch.path().join(key);
        let _ = std::fs::remove_file(&at);
        let _ = std::os::unix::fs::symlink(&elsewhere, &at);

        assert!(homepage.panels().await.is_err(), "{key}");
        assert!(homepage.apply(&panels()).await.is_err(), "{key}");
        assert_eq!(
            std::fs::read_to_string(&elsewhere).ok().as_deref(),
            Some(SHIPPED),
            "{key}"
        );
    }
}

#[test]
fn either_file_declared_unmanaged_is_the_dashboard_unmanaged() {
    let declared = |area: &str| vec![(area.to_owned(), "mine".to_owned())];

    assert!(unmanaged(&declared("config/homepage")));
    assert!(unmanaged(&declared(WIDGETS)));
    assert!(!unmanaged(&declared("config/caddy")));
    assert!(!unmanaged(&[]));
}
