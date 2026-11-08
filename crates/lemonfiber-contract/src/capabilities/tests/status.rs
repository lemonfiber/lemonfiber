//! `status.watch` across its contract: one script run against a watcher in process and
//! through the contract, which must answer alike and be told alike.

use async_trait::async_trait;
use lemonfiber_ports::service::{
    Beat, Covers, Failure, Instead, Monitor, MonitorKind, MonitorState, Monitoring, Provision,
    Provisioned, Reach,
};

use super::{crosses_alike, watch, Upstream};

#[async_trait]
impl Monitoring for Upstream {
    async fn reachable(&self) -> Result<Reach, Failure> {
        self.tell("reachable".to_owned());
        Ok(Reach {
            reachable: false,
            why: Some("its database is locked".to_owned()),
        })
    }
    async fn monitors(&self) -> Result<Vec<Monitor>, Failure> {
        self.tell("monitors".to_owned());
        Ok(vec![
            Monitor {
                id: "7".to_owned(),
                tag: Some(Covers::Job("guard".to_owned()).tag()),
                name: "The guard".to_owned(),
                kind: MonitorKind::Heartbeat,
                target: "guard".to_owned(),
                active: true,
                heartbeat: Some(Beat {
                    every_s: 60,
                    grace_s: 30,
                    last_check_in: Some(1_799_000_000),
                }),
                state: MonitorState::Pending,
                since: 1_799_000_060,
                last_reading: None,
            },
            Monitor {
                id: "8".to_owned(),
                tag: None,
                name: "The router".to_owned(),
                kind: MonitorKind::Ping,
                target: "192.168.1.1".to_owned(),
                active: false,
                heartbeat: None,
                state: MonitorState::Paused,
                since: 1_799_000_000,
                last_reading: Some(1_799_000_000),
            },
        ])
    }
    async fn provision(&self, provision: &Provision) -> Result<Provisioned, Failure> {
        self.tell(format!("provision {provision:?}"));
        Ok(Provisioned {
            id: provision.tag.len().to_string(),
            instead: (provision.kind == MonitorKind::Keyword).then(|| Instead {
                watches: provision.target.clone(),
                why: "it checks the status alone".to_owned(),
            }),
        })
    }
    async fn check_in(&self, tag: &str) -> Result<(), Failure> {
        self.tell(format!("check_in {tag}"));
        match Covers::read(tag) {
            Some(Covers::Job(_)) => Ok(()),
            Some(_) | None => Err(Failure::Refused {
                service: "upstream".to_owned(),
                detail: format!("no heartbeat holds {tag}"),
            }),
        }
    }
    async fn retire(&self, tag: &str) -> Result<(), Failure> {
        self.tell(format!("retire {tag}"));
        if Covers::read(tag).is_some() {
            Ok(())
        } else {
            Err(Failure::Disallowed {
                service: "upstream".to_owned(),
                detail: format!("{tag} is not lemonfiber's"),
            })
        }
    }
}

/// A result as the script writes it down: a failure by its kind and words alone, since the
/// service it names is the one asked rather than the one that answered.
fn said<T: std::fmt::Debug>(result: Result<T, Failure>) -> String {
    match result {
        Ok(answer) => format!("{answer:?}"),
        Err(Failure::Refused { detail, .. }) => format!("refused: {detail}"),
        Err(Failure::Disallowed { detail, .. }) => format!("disallowed: {detail}"),
        Err(failure) => format!("{failure}"),
    }
}

/// Every operation of the capability, each answer written down.
async fn script<M: Monitoring>(watcher: &M) -> Vec<String> {
    let provision = |kind| Provision {
        tag: Covers::EntryPoint("jellyfin".to_owned()).tag(),
        kind,
        target: "https://watch.house.lan".to_owned(),
        every_s: 60,
        grace_s: Some(30),
        retries: 3,
    };
    vec![
        said(watcher.reachable().await),
        said(watcher.monitors().await),
        said(watcher.provision(&provision(MonitorKind::Http)).await),
        said(watcher.provision(&provision(MonitorKind::Keyword)).await),
        said(watcher.check_in("lemonfiber:job:guard").await),
        said(watcher.check_in("lemonfiber:service:sonarr").await),
        said(watcher.retire("lemonfiber:service:sonarr").await),
        said(watcher.retire("router").await),
    ]
}

#[tokio::test]
async fn a_watcher_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(
        script,
        watch::Adapter,
        [
            "reachable",
            "monitors",
            "provision",
            "provision",
            "check_in",
            "check_in",
            "retire",
            "retire"
        ]
    );
}
