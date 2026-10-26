use lemonfiber_core::ports::http::{Method, Request};
use lemonfiber_core::ports::service::RootFolder;

use super::{answering, curating, told, ALL, FILM, MUSIC, TELEVISION};

#[test]
fn each_moved_plugin_is_its_curators_with_an_adapter_curating_in_front_of_it() {
    for curator in ALL {
        let installed = curator.installed();
        assert_eq!(
            installed.as_ref().map(|one| one.plugin.as_str()),
            Some(curator.plugin)
        );
        assert!(
            installed.is_some_and(|one| one.services.iter().any(|placed| {
                placed.service == curator.adapter
                    && placed.fronts.as_deref() == Some(curator.plugin)
                    && placed
                        .speaks
                        .iter()
                        .any(|spoken| spoken == "library.curate@1")
            })),
            "{}",
            curator.plugin
        );
    }
}

#[test]
fn each_curator_is_asked_on_a_port_of_its_own() {
    assert_eq!(
        curating(TELEVISION, "root_folders"),
        "127.0.0.1:8101/lemonfiber/library.curate/v1/root_folders"
    );
    assert_eq!(
        curating(FILM, "queue"),
        "127.0.0.1:8102/lemonfiber/library.curate/v1/queue"
    );
    assert_eq!(
        curating(MUSIC, "identity"),
        "127.0.0.1:8103/lemonfiber/library.curate/v1/identity"
    );
}

#[test]
fn what_a_curator_is_told_is_read_back_from_what_was_posted_to_it_alone() {
    let folder = RootFolder {
        path: "/data/media/tv".to_owned(),
        media_type: "tv".to_owned(),
    };
    let posted = |port: u16| Request {
        method: Method::Post,
        url: format!("http://127.0.0.1:{port}/lemonfiber/library.curate/v1/register_root_folder"),
        headers: Vec::new(),
        body: Some(serde_json::json!({ "folder": folder }).to_string()),
        pinned: None,
    };
    let sent = [posted(TELEVISION.port), posted(FILM.port)];

    let told: Vec<RootFolder> = told(&sent, TELEVISION, "register_root_folder", "folder");

    assert_eq!(told, vec![folder.clone()]);
    let (route, _) = answering(FILM, "root_folders", &Vec::<RootFolder>::new());
    assert_eq!(route, curating(FILM, "root_folders"));
}
