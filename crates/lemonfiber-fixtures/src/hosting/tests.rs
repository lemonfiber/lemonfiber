use super::{Failure, Held, Host, Hosted, Hosting, Manager, Placed, Program, Standing};
use std::path::PathBuf;

fn a_command() -> Hosted {
    Hosted {
        name: "watch".to_owned(),
        program: PathBuf::from("/usr/local/bin/lemonfiber"),
        arguments: vec!["watch".to_owned(), "full".to_owned()],
        output: PathBuf::from("/records/watch.log"),
        about: "guards the data location".to_owned(),
    }
}

#[tokio::test]
async fn what_was_installed_is_what_it_afterwards_holds() {
    let hosting = Hosting::with(Manager::Launchd);
    assert_eq!(hosting.manager(), Manager::Launchd);
    assert_eq!(hosting.standing("watch").await, Ok(Held::absent()));

    assert_eq!(
        hosting.place(&a_command()).await,
        Ok(Placed {
            definition: PathBuf::from("/services/lemonfiber-watch"),
            started: true,
        })
    );
    assert_eq!(hosting.placed(), vec![a_command()]);
    assert_eq!(
        hosting.standing("watch").await,
        Ok(Held {
            standing: Standing::Running,
            definition: Some(PathBuf::from("/services/lemonfiber-watch")),
            program: Some(Program {
                at: PathBuf::from("/usr/local/bin/lemonfiber"),
                present: true,
            }),
            runs: Some("watch full".to_owned()),
            output: Some(PathBuf::from("/records/watch.log")),
        })
    );
}

#[tokio::test]
async fn what_is_taken_back_is_named_and_then_gone() {
    let hosting = Hosting::with(Manager::Systemd);
    assert!(hosting.place(&a_command()).await.is_ok());
    assert_eq!(
        hosting.withdraw("watch").await,
        Ok(vec![PathBuf::from("/services/lemonfiber-watch")])
    );
    assert_eq!(hosting.withdrawn(), vec!["watch".to_owned()]);
    assert_eq!(hosting.standing("watch").await, Ok(Held::absent()));
    assert_eq!(hosting.withdraw("watch").await, Ok(Vec::new()));
}

#[tokio::test]
async fn a_platform_with_no_manager_answers_nothing_at_all() {
    let hosting = Hosting::unsupported();
    assert_eq!(hosting.manager(), Manager::Unsupported);
    assert_eq!(hosting.place(&a_command()).await, Err(Failure::Unhostable));
    assert_eq!(hosting.standing("watch").await, Err(Failure::Unhostable));
    assert_eq!(hosting.withdraw("watch").await, Err(Failure::Unhostable));
}

#[tokio::test]
async fn a_manager_that_refuses_still_answers_what_it_holds() {
    let hosting = Hosting::refusing(Manager::Launchd, "Load failed: 5");
    let refusal = || Failure::Refused {
        manager: "launchd",
        reason: "Load failed: 5".to_owned(),
    };
    assert_eq!(hosting.place(&a_command()).await, Err(refusal()));
    assert_eq!(hosting.withdraw("watch").await, Err(refusal()));
    assert_eq!(hosting.standing("watch").await, Ok(Held::absent()));
}

#[tokio::test]
async fn a_machine_can_be_built_already_holding_one() {
    let hosting = Hosting::holding(
        Manager::Systemd,
        "expiring",
        Hosting::installed("expiring", Standing::Stopped),
    );
    assert_eq!(
        hosting.standing("expiring").await,
        Ok(Hosting::installed("expiring", Standing::Stopped))
    );

    let gone: Held = Hosting::orphaned("expiring");
    assert!(gone.orphaned());
    assert_eq!(gone.standing, Standing::Stopped);
    assert!(gone.definition.is_some());
}
