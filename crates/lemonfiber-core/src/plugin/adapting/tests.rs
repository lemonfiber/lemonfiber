use super::{as_adapter, handed_over, owner, running_as, HANDED_TO};

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
