use super::{filled, placeholders, refused, Form};
use crate::refusing::tests::INSTALLABLE;
use crate::Manifest;

/// The fixture with a secret of its own service and one of another's.
fn manifest() -> Option<Manifest> {
    let text = INSTALLABLE.to_owned()
        + "\n[[secret]]\nid     = \"api-key\"\nof     = \"komga\"\nminted = true\nwhy    = \"Sign \
           in\"\n\n[[secret]]\nid  = \"curator\"\nof  = \"sonarr\"\nwhy = \"Ask the curator\"\n";
    Manifest::from_toml(&text).ok()
}

#[test]
fn every_placeholder_is_read_in_order_with_its_form() {
    let text = "a = {{secret:one}}\nb = {{secret:two|pbkdf2}}\nc = {{other}}\n";
    let named = placeholders(text).unwrap_or_default();
    let read: Vec<(&str, Form)> = named.iter().map(|one| (one.secret, one.form)).collect();
    assert_eq!(read, vec![("one", Form::Plain), ("two", Form::Pbkdf2)]);
    assert_eq!(
        named.first().and_then(|one| text.get(one.span.clone())),
        Some("{{secret:one}}")
    );
}

#[test]
fn a_placeholder_that_never_closes_names_no_secret_or_asks_another_form_is_refused() {
    for (text, says) in [
        ("key = {{secret:one", "never closes"),
        ("key = {{secret:}}", "names no secret"),
        ("key = {{secret:one|sha1}}", "the form sha1"),
    ] {
        let refused = placeholders(text).err().unwrap_or_default();
        assert!(refused.contains(says), "{text}: {refused}");
    }
}

#[test]
fn filling_writes_each_placeholder_and_leaves_everything_else_as_it_was() {
    let filled = filled(
        "[a]\nkey = {{secret:one}} # {{secret:two|pbkdf2}}\n",
        |one| {
            Ok(match one.form {
                Form::Plain => format!("<{}>", one.secret),
                Form::Pbkdf2 => format!("<hash of {}>", one.secret),
            })
        },
    );
    assert_eq!(filled.as_deref(), Ok("[a]\nkey = <one> # <hash of two>\n"));
}

#[test]
fn a_starting_file_names_only_secrets_of_the_plugins_own_services() {
    let read = manifest();
    assert!(read.is_some(), "the fixture is not a manifest");
    let Some(manifest) = read else {
        return;
    };
    assert!(refused(&manifest, "komga", "app.ini", b"key = {{secret:api-key}}\n").is_empty());
    assert!(refused(&manifest, "komga", "image.bin", &[0xff, 0xfe, 0x00]).is_empty());
    for (bytes, says) in [
        (
            &b"key = {{secret:missing}}"[..],
            "no [[secret]] of this manifest declares",
        ),
        (
            &b"key = {{secret:curator}}"[..],
            "a credential that crosses",
        ),
        (&b"key = {{secret:api-key|md5}}"[..], "the form md5"),
        (&b"\xff{{secret:api-key}}"[..], "not UTF-8"),
    ] {
        let said = refused(&manifest, "komga", "app.ini", bytes);
        assert!(
            said.iter()
                .any(|one| one.location == "config/komga/app.ini" && one.message.contains(says)),
            "{says}: {said:?}"
        );
    }
}
