#[test]
fn losing_hardlinks_is_stated_as_what_it_costs_rather_than_what_it_is() {
    // "Hardlinks unsupported" means nothing to most operators. What it means is
    // three concrete things, and all three have to be there — an operator
    // weighing whether to accept a location is weighing exactly these.
    let said = super::COPY_CONSEQUENCE.to_lowercase();
    assert!(said.contains("minutes"), "import time: {said}");
    assert!(said.contains("twice the disk"), "disk usage: {said}");
    assert!(said.contains("seed"), "seeding: {said}");
    // And it never leads with the property itself.
    assert!(!said.contains("hardlink"), "a filesystem property: {said}");
}

/// A link a container planted where a probe might go is never followed: the file it
/// points at is neither emptied nor linked, and the probe still answers.
#[cfg(unix)]
#[tokio::test]
async fn a_link_planted_where_a_probe_might_go_is_never_followed() {
    let dir = lemonfiber_fixtures::scratch::Scratch::new("probe-planted");
    let operator = dir.join("operator.env");
    let _ = std::fs::write(&operator, "SECRET=kept\n");
    let data = dir.join("data");
    let _ = std::fs::create_dir_all(&data);
    let _ = std::os::unix::fs::symlink(&operator, data.join(".lemonfiber-hardlink-probe"));

    let linked = super::test_link(&lemonfiber_adapters::Disk, &data).await;

    assert!(
        matches!(linked, super::Linked::Yes { .. }),
        "got: {linked:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&operator).ok().as_deref(),
        Some("SECRET=kept\n")
    );
}

/// Every probe takes names of its own, so two at once never clear each other's files,
/// and neither leaves anything behind.
#[test]
fn every_probe_takes_names_of_its_own() {
    let dir = std::path::Path::new("/data");
    let (first, first_link) = super::names(dir);
    let (second, _) = super::names(dir);

    assert_ne!(first, second);
    assert_eq!(
        first_link,
        first.with_file_name(format!(
            "{}.link",
            first.file_name().unwrap_or_default().to_string_lossy()
        ))
    );
    assert!(first.file_name().is_some_and(|name| name
        .to_string_lossy()
        .starts_with(".lemonfiber-hardlink-probe")));
}
