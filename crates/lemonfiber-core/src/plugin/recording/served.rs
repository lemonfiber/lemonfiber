//! One service served for its recordings: started fresh from its pin, waited on until
//! its health passes, first-run, asked each request, and removed whatever happened.

use std::path::{Path, PathBuf};
use std::time::Duration;

use lemonfiber_contract::capabilities::FOLDER;
use lemonfiber_manifest::container::{Secret, STREAM_GUARD};
use lemonfiber_plugin::{Health, HealthKind};
use tokio::time::Instant;

use super::addressed::Addressed;
use super::asked::{answered, Item};
use super::scrubbed::Scrub;
use super::stood_in::{StandIns, STAND_INS};
use super::{Failed, Recorder};
use crate::plugin::container::LOOPBACK;
use crate::plugin::installed::Placed;
use crate::plugin::program::{contained, Contained, Operator, Starting, Throwaway};
use crate::plugin::recorded::Answer;
use crate::ports::http::{Http, Method, Request};
use crate::ports::serving::{Bound, Serve, Served, Standing};

/// How often a service still coming up is looked at again.
pub(super) const POLL: Duration = Duration::from_millis(500);

/// How long a service may take to become healthy where its health names no limit.
const HEALTHY_WITHIN: Duration = Duration::from_secs(60);

/// How many of the last lines a failed service's container wrote are shown.
pub(super) const LOG_LINES: usize = 50;

/// The statuses an application answers a caller it will not serve, which tell it is
/// up as surely as an answer does.
const REFUSALS: [u16; 2] = [401, 403];

/// Where the library a service takes lands inside its container.
const LIBRARY: &str = "/data";

/// The library a service takes, beside its starting files.
const DATA: &str = "data";

/// What the step that makes a service's further mounts is called where it fails.
pub(super) const MOUNTS: &str = "mounts";

/// What each further mount a service is given is named beside its starting files, before
/// its place among them.
const MOUNTED: &str = "mount-";

/// What one served service answered each of its requests, scrubbed.
pub(super) async fn answers(
    recorder: Recorder<'_>,
    root: &Path,
    placed: &Placed,
    health: Option<&Health>,
    items: &[&Item<'_>],
    secrets: &[Secret],
) -> Result<Vec<Answer>, Failed> {
    let failing = |step: &'static str| {
        move |why: String| Failed {
            service: placed.service.clone(),
            step: step.to_owned(),
            why: crate::text::plain(&why),
            log: Vec::new(),
        }
    };
    let copy = Throwaway::of(
        Some(Starting {
            root,
            service: &placed.service,
        }),
        recorder.random,
    )
    .map_err(failing("starting files"))?;
    certified(placed, &copy).map_err(failing("certificate"))?;
    let data = library(placed, &copy).map_err(failing("data"))?;
    mountable(placed, &copy).map_err(failing(MOUNTS))?;
    let mut stand_ins = StandIns::of(secrets, recorder.random).map_err(failing(STAND_INS))?;
    let stood = stand_ins
        .settings(placed, recorder.random)
        .map_err(failing(STAND_INS))?;
    let settled = settled(placed, &copy, data.as_deref());
    let serve = Serve {
        env: settled
            .env
            .iter()
            .chain(&stood)
            .chain(&stand_ins.secrets(placed))
            .cloned()
            .collect(),
        ..settled
    };
    let served = recorder
        .serving
        .serve(&serve)
        .await
        .map_err(|unran| failing("container")(unran.to_string()))?;
    let seeded: Vec<String> = copy
        .made()
        .map(str::to_owned)
        .chain(stand_ins.made.iter().cloned())
        .collect();
    let mut scrub = Scrub::of(&seeded, Some(&served));
    let reaching = Reaching {
        placed,
        served: &served,
    };
    let outcome = asked(recorder, health, &reaching, items, &mut scrub).await;
    let finished = match outcome {
        Ok(answers) => Ok(answers),
        Err((step, why)) => {
            let log = recorder.serving.log(&served, LOG_LINES).await;
            Err(Failed {
                service: placed.service.clone(),
                step: scrub.text(&step),
                why: crate::text::plain(&scrub.text(&why)),
                log: scrub.text(&log).lines().map(crate::text::plain).collect(),
            })
        }
    };
    recorder.serving.removed(&served).await;
    finished
}

/// A throwaway certificate and its key written into `copy` as the core writes them for a
/// filler of a capability it hands a certificate, and readable by whoever that filler runs
/// as, where `placed` provides one; nothing otherwise.
pub(super) fn certified(placed: &Placed, copy: &Throwaway) -> Result<(), String> {
    if !placed.provides.iter().any(|name| name == STREAM_GUARD) {
        return Ok(());
    }
    crate::certificate::replaced(copy.path()).map_err(|unkept| unkept.to_string())?;
    crate::certificate::FILES
        .iter()
        .try_for_each(|named| copy.readable(named))
}

/// An empty library beside `copy` holding the folder the conformance cases file into,
/// where `placed` takes one; nothing otherwise.
pub(super) fn library(placed: &Placed, copy: &Throwaway) -> Result<Option<PathBuf>, String> {
    if !placed.takes_data {
        return Ok(None);
    }
    let data = copy.emptied(Path::new(DATA))?;
    let folder = Path::new(FOLDER)
        .strip_prefix(LIBRARY)
        .unwrap_or(Path::new(""));
    copy.emptied(&Path::new(DATA).join(folder))?;
    Ok(Some(data))
}

/// Each further mount install gives `placed`, as the directory beside `copy` it is served
/// from and where it lands.
fn mounted(placed: &Placed, copy: &Throwaway) -> Vec<Bound> {
    crate::project::entry::mounts(&placed.fields.mount, placed.takes_data)
        .enumerate()
        .map(|(at, mount)| Bound {
            source: copy.beside(&mount_named(at)),
            target: mount.to.clone(),
            read_only: mount.ro,
        })
        .collect()
}

/// An empty directory beside `copy` for each further mount install gives `placed`, open to
/// whoever the service is.
pub(in crate::plugin) fn mountable(placed: &Placed, copy: &Throwaway) -> Result<(), String> {
    (0..crate::project::entry::mounts(&placed.fields.mount, placed.takes_data).count())
        .try_for_each(|at| copy.emptied(&mount_named(at)).map(drop))
}

/// What the directory the further mount at `at` is served from is named beside the copy.
fn mount_named(at: usize) -> PathBuf {
    PathBuf::from(format!("{MOUNTED}{at}"))
}

/// The container a service's recordings are taken from: as lemonfiber runs it, with
/// throwaway starting files and library and its one port on the loopback interface.
pub(super) fn settled(placed: &Placed, copy: &Throwaway, data: Option<&Path>) -> Serve {
    settled_as(placed, copy, data, &Operator::recording())
}

/// The same container, run as `operator` rather than as the entry writes it where nothing
/// is set.
pub(in crate::plugin) fn settled_as(
    placed: &Placed,
    copy: &Throwaway,
    data: Option<&Path>,
    operator: &Operator,
) -> Serve {
    let Contained {
        env,
        user,
        security_opt,
        cap_drop,
        cap_add,
        pids_limit,
        read_only,
        memory,
        devices,
    } = contained(placed, operator);
    let mut binds = vec![Bound {
        source: copy.path().to_path_buf(),
        target: crate::project::entry::configuration(&placed.config_path).to_owned(),
        read_only: placed.fields.config_ro,
    }];
    binds.extend(data.map(|data| Bound {
        source: data.to_path_buf(),
        target: LIBRARY.to_owned(),
        read_only: false,
    }));
    binds.extend(mounted(placed, copy));
    Serve {
        reaching: Vec::new(),
        image: format!("{}@{}", placed.image, placed.digest),
        env,
        command: Vec::new(),
        user,
        security_opt,
        cap_drop,
        cap_add,
        pids_limit,
        read_only,
        memory,
        devices,
        binds,
        tmpfs: crate::plugin::container::tmpfs(placed).cloned().collect(),
        listens: inside(placed),
        contract: placed
            .contract_port()
            .filter(|port| Some(*port) != inside(placed)),
        alias: None,
    }
}

/// What asking the served service reaches it through.
pub(super) struct Reaching<'a> {
    /// The service, as installing it places it.
    pub(super) placed: &'a Placed,
    /// The container serving it.
    pub(super) served: &'a Served,
}

/// The port the service answers on inside its network.
pub(super) fn inside(placed: &Placed) -> Option<u16> {
    placed.listens.or_else(|| {
        placed
            .reached
            .as_ref()
            .map(crate::plugin::installed::Reached::port)
    })
}

/// Every answer, once the service is healthy and its first run done; or the step that
/// failed and why.
async fn asked(
    recorder: Recorder<'_>,
    health: Option<&Health>,
    reaching: &Reaching<'_>,
    items: &[&Item<'_>],
    scrub: &mut Scrub,
) -> Result<Vec<Answer>, (String, String)> {
    let served = reaching.served;
    let plain = Addressed::to((recorder.transport)(), LOOPBACK, inside(reaching.placed));
    healthy(recorder.serving, plain.as_ref(), health, served)
        .await
        .map_err(|why| ("health".to_owned(), why))?;
    if let Some(operators) = items.iter().find(|item| item.keyed) {
        return Err((
            "first run".to_owned(),
            format!(
                "{} is asked with the operator's credential, and no first run lemonfiber \
                 performs for this service leaves it holding one",
                operators.named
            ),
        ));
    }
    let port = served.port;
    let mut answers = Vec::new();
    for item in items {
        let step = format!("asking {}", item.named);
        let port = port.ok_or_else(|| {
            (
                step.clone(),
                "it publishes no port, so there is nowhere to ask it".to_owned(),
            )
        })?;
        let answer = answered(plain.as_ref(), port, item)
            .await
            .map_err(|why| (step.clone(), why))?;
        let answer = scrub.answer(answer);
        let text = serde_json::to_string(&answer).unwrap_or_default();
        if scrub.holds(&text) {
            return Err((
                format!("scrubbing {}", item.named),
                "what it answered still holds a credential or the container's identity once \
                 scrubbed, so it is not written"
                    .to_owned(),
            ));
        }
        answers.push(answer);
    }
    Ok(answers)
}

/// How long the service is given to come up: its health's own limit, or the recorder's.
pub(super) fn allowed(health: Option<&Health>) -> Duration {
    health
        .and_then(|health| health.timeout_s)
        .map_or(HEALTHY_WITHIN, |seconds| {
            Duration::from_secs(u64::from(seconds))
        })
}

/// Wait until the service's health passes, or say why it did not within its limit.
pub(in crate::plugin) async fn healthy(
    serving: &dyn crate::ports::serving::Serving,
    http: &dyn Http,
    health: Option<&Health>,
    served: &Served,
) -> Result<(), String> {
    healthy_at(serving, http, health, served, served.port).await
}

/// Wait until `served`'s health passes at `port` on the loopback interface, or say why it
/// will not.
pub(in crate::plugin) async fn healthy_at(
    serving: &dyn crate::ports::serving::Serving,
    http: &dyn Http,
    health: Option<&Health>,
    served: &Served,
    port: Option<u16>,
) -> Result<(), String> {
    let within = allowed(health);
    let until = Instant::now() + within;
    loop {
        let standing = serving
            .standing(served)
            .await
            .map_err(|unran| unran.to_string())?;
        if let Standing::Exited(code) = standing {
            return Err(format!("it exited {code} before its health passed"));
        }
        if passes(http, health, port, standing).await {
            return Ok(());
        }
        if Instant::now() >= until {
            return Err(format!(
                "its health did not pass within {} seconds",
                within.as_secs()
            ));
        }
        tokio::time::sleep(POLL).await;
    }
}

/// Whether the service's health passes now: its path answering over HTTP without an
/// error or with the application's own refusal, which no port proxy can give, anything
/// answering over TCP, and the container's own word otherwise.
async fn passes(
    http: &dyn Http,
    health: Option<&Health>,
    port: Option<u16>,
    standing: Standing,
) -> bool {
    let kind = health.map_or(HealthKind::Container, |health| health.kind);
    let (HealthKind::Http | HealthKind::Tcp, Some(port)) = (kind, port) else {
        return matches!(standing, Standing::Healthy | Standing::Running);
    };
    let path = health
        .and_then(|health| health.path.as_deref())
        .filter(|path| lemonfiber_plugin::refusing::carried::is_route(path))
        .unwrap_or("/");
    let request = Request {
        method: Method::Get,
        url: format!("http://{LOOPBACK}:{port}{path}"),
        headers: Vec::new(),
        body: None,
        pinned: None,
    };
    match (kind, http.send(&request).await) {
        (HealthKind::Http, Ok(response)) => {
            (200..400).contains(&response.status) || REFUSALS.contains(&response.status)
        }
        (_, Ok(_)) => true,
        (_, Err(_)) => false,
    }
}

#[cfg(test)]
mod tests;
