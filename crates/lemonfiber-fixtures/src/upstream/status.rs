//! The upstream as a status watcher.

use async_trait::async_trait;
use lemonfiber_ports::service::{
    Beat, Covers, Failure, Instead, Monitor, MonitorKind, MonitorState, Monitoring, Provision,
    Provisioned, Reach,
};

use super::Upstream;

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
