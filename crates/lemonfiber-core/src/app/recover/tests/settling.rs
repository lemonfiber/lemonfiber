use super::super::{carrying_out, undo};
use super::scratch;
use crate::credential::fingerprint;
use crate::error::codes::setup::UNKEPT;
use crate::journal::{Action, Undo};

fn rehold(previous: Option<&str>, wrote: Option<&str>) -> Undo {
    Undo {
        target: "komga:token".to_owned(),
        action: Action::Rehold {
            name: "komga:token".to_owned(),
            earlier: previous.map(fingerprint),
            kept: wrote.map(fingerprint),
            filed: None,
        },
    }
}

fn keep(secrets: &std::path::Path, value: &str) {
    let keeping = [crate::settings::Keeping {
        name: "komga:token",
        value,
        from: crate::settings::Source::Recipe,
        at: 1,
    }];
    assert!(crate::settings::kept(secrets, &keeping, &|_| Ok(())).is_ok());
}

#[test]
fn a_secret_kept_by_a_change_is_forgotten_when_it_goes_back() {
    let dir = scratch("rehold-forget");
    let env = dir.join(".env");
    keep(&dir.join("secrets.json"), "t0k3n");
    let carried = carrying_out(
        &lemonfiber_adapters::Disk,
        &[rehold(None, Some("t0k3n"))],
        &env,
        Vec::new(),
    );
    assert_eq!(carried.map(|one| one.done.len()).ok(), Some(1));
    let held = crate::settings::Secrets::at(&dir.join("secrets.json")).unwrap_or_default();
    assert_eq!(held.value("komga:token"), None);
}

#[test]
fn a_secret_whose_earlier_value_was_not_kept_is_named_and_left() {
    let dir = scratch("rehold-unkept");
    let env = dir.join(".env");
    keep(&dir.join("secrets.json"), "second");
    let refused = undo(
        &lemonfiber_adapters::Disk,
        &[rehold(Some("first"), Some("second"))],
        &env,
        Vec::new(),
    );
    assert!(
        matches!(&refused, Err(problem) if problem.code == UNKEPT
            && problem.detail.as_deref() == Some("komga:token")),
        "{refused:?}"
    );
}

#[test]
fn a_setting_recorded_by_a_change_is_cleared_when_it_goes_back() {
    let dir = scratch("resettle");
    let env = dir.join(".env");
    assert!(std::fs::create_dir_all(&dir).is_ok());
    assert!(std::fs::write(
        dir.join("settings.json"),
        r#"{"schema":1,"written_by":"","values":{"komga:mode":"fast"}}"#
    )
    .is_ok());
    let resettle = Undo {
        target: "komga:mode".to_owned(),
        action: Action::Resettle {
            name: "komga:mode".to_owned(),
            value: None,
            recorded: Some("fast".to_owned()),
        },
    };
    assert!(undo(&lemonfiber_adapters::Disk, &[resettle], &env, Vec::new()).is_ok());
    let left = crate::settings::Settings::at(&dir.join("settings.json")).unwrap_or_default();
    assert_eq!(left.value("komga:mode"), None);
}

#[test]
fn a_setting_chosen_since_is_left_and_named() {
    let dir = scratch("resettle-theirs");
    let env = dir.join(".env");
    assert!(std::fs::create_dir_all(&dir).is_ok());
    assert!(std::fs::write(
        dir.join("settings.json"),
        r#"{"schema":1,"written_by":"","values":{"komga:mode":"theirs"}}"#
    )
    .is_ok());
    let resettle = Undo {
        target: "komga:mode".to_owned(),
        action: Action::Resettle {
            name: "komga:mode".to_owned(),
            value: None,
            recorded: Some("fast".to_owned()),
        },
    };
    let carried = carrying_out(&lemonfiber_adapters::Disk, &[resettle], &env, Vec::new());
    assert_eq!(
        carried.map(|one| one.theirs).ok(),
        Some(vec!["komga:mode".to_owned()])
    );
}
