use super::Certificate;

fn declared(key: &str, certificate: &str, names: &[&str]) -> Certificate {
    Certificate {
        key: key.to_owned(),
        certificate: certificate.to_owned(),
        names: names.iter().map(|name| (*name).to_owned()).collect(),
    }
}

#[test]
fn a_certificate_beneath_the_configuration_directory_for_host_names_breaks_nothing() {
    let certificate = declared(
        "tls/key.pem",
        "tls/certificate.pem",
        &["door", "door.home-1"],
    );
    assert_eq!(certificate.broken(&[]), Vec::<String>::new());
}

#[test]
fn a_path_that_leaves_or_is_not_one_is_refused() {
    for path in [
        "/etc/key.pem",
        "../key.pem",
        "tls/../key.pem",
        "",
        "tls//key.pem",
        "{{key}}",
    ] {
        let found = declared(path, "certificate.pem", &["door"]).broken(&[]);
        assert_eq!(
            found,
            vec![format!(
                "{path} is not a relative path beneath its configuration directory"
            )],
            "{path}"
        );
    }
}

#[test]
fn one_path_for_both_is_refused() {
    assert_eq!(
        declared("tls.pem", "tls.pem", &["door"]).broken(&[]),
        vec!["tls.pem is both the key's path and the certificate's".to_owned()]
    );
}

#[test]
fn no_names_or_too_many_are_refused() {
    let many: Vec<String> = (0..=super::MOST_NAMES).map(|at| format!("n{at}")).collect();
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    for (names, count) in [(Vec::new(), 0), (many, super::MOST_NAMES + 1)] {
        assert_eq!(
            declared("key.pem", "certificate.pem", &names).broken(&[]),
            vec![format!(
                "names {count} names, where a certificate is made out to one to {}",
                super::MOST_NAMES
            )]
        );
    }
}

#[test]
fn a_name_that_is_no_host_name_is_refused() {
    let long = "a".repeat(254);
    for name in [
        "Door",
        "door name",
        "-door",
        "door-",
        "door..home",
        "",
        long.as_str(),
        "d_oor",
    ] {
        let found = declared("key.pem", "certificate.pem", &[name]).broken(&[]);
        assert_eq!(found.len(), 1, "{name:?}");
        assert!(found
            .iter()
            .all(|one| one.starts_with(&format!("{name:?} is not a host name"))));
    }
}

#[test]
fn a_stream_guard_declares_none() {
    assert_eq!(
        declared("key.pem", "certificate.pem", &["door"]).broken(&["stream.guard".to_owned()]),
        vec!["it provides stream.guard, whose certificate the core makes for it".to_owned()]
    );
}
