use std::path::PathBuf;

use super::{Export, Refused, Storage};

fn nfs(server: &str, path: &str) -> Storage {
    Storage::Nfs(Export {
        server: server.to_owned(),
        path: path.to_owned(),
    })
}

#[test]
fn a_path_is_bound_and_a_blank_names_nothing() {
    assert_eq!(
        Storage::read(" /srv/media "),
        Ok(Some(Storage::Bind(PathBuf::from("/srv/media"))))
    );
    assert_eq!(Storage::read("  "), Ok(None));
    assert_eq!(
        Storage::read("/srv/media")
            .ok()
            .flatten()
            .as_ref()
            .and_then(Storage::on_this_machine),
        Some(&PathBuf::from("/srv/media"))
    );
    assert_eq!(
        Storage::read("/srv/media")
            .ok()
            .flatten()
            .as_ref()
            .and_then(Storage::export),
        None
    );
}

#[test]
fn an_export_is_read_by_its_server_and_its_path() {
    for (written, server, path) in [
        (
            "nfs://nas.local/volume1/media",
            "nas.local",
            "/volume1/media",
        ),
        ("nfs://192.168.1.20/export", "192.168.1.20", "/export"),
        ("nfs://[fd00::20]/export/data", "fd00::20", "/export/data"),
        ("nfs://nas/", "nas", "/"),
    ] {
        let read = Storage::read(written);
        assert_eq!(read, Ok(Some(nfs(server, path))), "{written}");
        let export = read.ok().flatten();
        assert_eq!(
            export
                .as_ref()
                .and_then(Storage::export)
                .map(Export::written),
            Some(written.to_owned())
        );
        assert_eq!(export.as_ref().and_then(Storage::on_this_machine), None);
    }
}

#[test]
fn a_server_that_is_no_host_name_or_address_is_refused_naming_it() {
    for (written, server) in [
        ("nfs:///export", ""),
        ("nfs://nas,soft/export", "nas,soft"),
        ("nfs://nas local/export", "nas local"),
        ("nfs://-nas/export", "-nas"),
        ("nfs://nas-/export", "nas-"),
        ("nfs://nas..local/export", "nas..local"),
        ("nfs://[fd00::20/export", "[fd00::20/export"),
        ("nfs://[nas]/export", "nas"),
    ] {
        assert_eq!(
            Storage::read(written),
            Err(Refused::Server(server.to_owned())),
            "{written}"
        );
    }
    assert!(Refused::Server("nas,soft".to_owned())
        .said()
        .contains("\"nas,soft\""));
    let long = format!("nfs://{}/export", "a".repeat(64));
    assert!(matches!(Storage::read(&long), Err(Refused::Server(_))));
}

#[test]
fn an_export_that_would_change_the_mount_options_is_refused_naming_it() {
    for (written, path) in [
        ("nfs://nas", ""),
        ("nfs://[fd00::20]export", "export"),
        ("nfs://nas/a,soft", "/a,soft"),
        ("nfs://nas/a b", "/a b"),
        ("nfs://nas/a\"b", "/a\"b"),
        ("nfs://nas/a'b", "/a'b"),
        ("nfs://nas/a\\b", "/a\\b"),
        ("nfs://nas/a\tb", "/a\tb"),
    ] {
        assert_eq!(
            Storage::read(written),
            Err(Refused::Export(path.to_owned())),
            "{written}"
        );
    }
    assert!(Refused::Export("/a,soft".to_owned())
        .said()
        .contains("\"/a,soft\""));
}
