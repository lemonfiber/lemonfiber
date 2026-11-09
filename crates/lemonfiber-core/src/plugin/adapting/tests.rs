use super::{as_adapter, handed_over, handed_over_whole, owner, running_as, HANDED_TO};

#[test]
fn an_adapter_runs_as_the_owner_of_what_lemonfiber_writes_and_never_as_root() {
    assert_eq!(as_adapter((1001, 121)), (1001, 121));
    assert_eq!(as_adapter((1000, 1000)), (1000, 1000));
    assert_eq!(as_adapter((0, 0)), HANDED_TO);
    assert_eq!(as_adapter((0, 1001)), HANDED_TO);
    assert_eq!(HANDED_TO, (1000, 1000));
}

#[test]
fn a_stack_written_by_its_operator_runs_its_adapters_as_them_and_hands_nothing_over() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("adapting-owner").kept();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(std::fs::create_dir_all(dir.join("config/one-adapter")).is_ok());
    let key = dir.join("config/one-adapter/lemonfiber.key");
    assert!(std::fs::write(&key, "key").is_ok());

    let operator = owner(&dir);
    assert!(operator.is_some());
    assert_eq!(running_as(&dir), operator.map(as_adapter));
    assert_eq!(handed_over(&key, &dir), Ok(()));
    assert_eq!(owner(&key), running_as(&dir));
    assert_eq!(running_as(&dir.join("nowhere")), None);
    assert_eq!(handed_over(&dir.join("nowhere"), &dir), Ok(()));
}

#[test]
fn a_whole_tree_is_handed_over_without_following_a_link_out_of_it() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("adapting-whole").kept();
    let _ = std::fs::remove_dir_all(&dir);
    let lent = dir.join("lent");
    assert!(std::fs::create_dir_all(lent.join("deeper")).is_ok());
    assert!(std::fs::write(lent.join("lemonfiber.key"), "key").is_ok());
    assert!(std::fs::write(lent.join("deeper/upstream.json"), "{}").is_ok());
    let outside = dir.join("outside");
    assert!(std::fs::write(&outside, "not the adapter's").is_ok());
    #[cfg(unix)]
    assert!(std::os::unix::fs::symlink(&outside, lent.join("out")).is_ok());

    let operator = owner(&lent);
    assert!(operator.is_some());
    let Some(operator) = operator else {
        return;
    };
    assert_eq!(handed_over_whole(&lent, operator), Ok(()));
    for beneath in [
        "",
        "lemonfiber.key",
        "deeper",
        "deeper/upstream.json",
        "out",
    ] {
        assert_eq!(owner(&lent.join(beneath)), Some(operator), "{beneath}");
    }
    assert!(handed_over_whole(&dir.join("nowhere"), operator).is_err());
}

/// A stack directory root owns runs its adapters as the fallback user, and what is written
/// for them is handed to that user, where whoever runs this may hand it over.
#[test]
fn what_is_written_beneath_a_stack_root_owns_is_handed_to_the_fallback_user() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("adapting-root").kept();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(std::fs::create_dir_all(&dir).is_ok());
    let key = dir.join("lemonfiber.key");
    assert!(std::fs::write(&key, "key").is_ok());
    let rooted = std::path::Path::new("/");
    assert_eq!(owner(rooted).map(|(uid, _)| uid), Some(0));
    assert_eq!(running_as(rooted), Some(HANDED_TO));

    let handed = handed_over(&key, rooted);

    let runner = owner(&key);
    if runner == Some(HANDED_TO) || runner.is_some_and(|(uid, _)| uid == 0) {
        assert_eq!(handed, Ok(()));
    } else {
        assert!(
            handed
                .as_ref()
                .is_err_and(|why| why.contains("could not be handed to 1000:1000")),
            "{handed:?}"
        );
    }
}
