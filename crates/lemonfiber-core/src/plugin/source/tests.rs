use std::path::PathBuf;

use super::Source;

fn git(url: &str, revision: Option<&str>) -> Source {
    Source::Git {
        url: url.to_owned(),
        revision: revision.map(str::to_owned),
    }
}

/// Anything that is neither an address git fetches from nor a plugin's id is a path.
#[test]
fn a_directory_is_a_path() {
    for written in [
        "./komga",
        "/home/ana/komga",
        "komga/plugin.toml",
        "komga@2",
        "Komga",
        "2komga",
        "komga-",
        "-komga",
        "komga--kuma",
        "komga_kuma",
        "",
    ] {
        assert_eq!(Source::named(written), Source::Path(PathBuf::from(written)));
    }
    assert!(!Source::named("./komga").is_git());
}

/// A bare word shaped as a plugin's id is a name for the catalogue to resolve.
#[test]
fn a_word_shaped_as_an_id_is_a_name() {
    for written in ["komga", "uptime-kuma", "a2", "plugin-2-go"] {
        assert_eq!(Source::named(written), Source::Name(written.to_owned()));
        assert!(!Source::named(written).is_git());
    }
}

/// An address names no revision unless one follows its last `@`.
#[test]
fn an_address_names_a_revision_after_its_last_at() {
    assert_eq!(
        Source::named("https://github.com/ana/plugin-komga"),
        git("https://github.com/ana/plugin-komga", None)
    );
    assert_eq!(
        Source::named("https://github.com/ana/plugin-komga@v1.2.0"),
        git("https://github.com/ana/plugin-komga", Some("v1.2.0"))
    );
    assert!(Source::named("https://github.com/ana/plugin-komga").is_git());
}

/// Neither the `user@` of an address nor git's own `git@host:` is a revision.
#[test]
fn a_user_in_an_address_is_not_a_revision() {
    assert_eq!(
        Source::named("https://ana@example.org/plugin-komga.git"),
        git("https://ana@example.org/plugin-komga.git", None)
    );
    assert_eq!(
        Source::named("git@github.com:ana/plugin-komga.git"),
        git("git@github.com:ana/plugin-komga.git", None)
    );
    assert_eq!(
        Source::named("git@github.com:ana/plugin-komga.git@main"),
        git("git@github.com:ana/plugin-komga.git", Some("main"))
    );
    assert_eq!(
        Source::named("git@github.com:plugin-komga@main"),
        git("git@github.com:plugin-komga", Some("main"))
    );
    assert_eq!(
        Source::named("ssh://git@example.org/plugin-komga"),
        git("ssh://git@example.org/plugin-komga", None)
    );
}

/// A trailing `@` names nothing, so the address is taken whole.
#[test]
fn a_trailing_at_names_no_revision() {
    assert_eq!(
        Source::named("https://github.com/ana/plugin-komga@"),
        git("https://github.com/ana/plugin-komga@", None)
    );
}

/// Every scheme git reads but https is named, and https and a path are not.
#[test]
fn every_scheme_but_https_is_named_as_written() {
    for (written, scheme) in [
        ("http://example.org/x", Some("http://")),
        ("ssh://git@example.org/x", Some("ssh://")),
        ("git://example.org/x", Some("git://")),
        ("git@example.org:x", Some("git@")),
        ("https://example.org/x", None),
        ("./komga", None),
    ] {
        assert_eq!(super::unspoken(written), scheme, "{written}");
    }
    assert!(Source::named("http://example.org/x").is_git());
}
