//! Turning the engine's replies into the port's vocabulary.
//!
//! Pure translation, kept apart from the daemon I/O in the parent module so the
//! two concerns read separately: a state or health enum into the port's own, a
//! status line into an exit code, a log line into its timestamp, a stats sample
//! into a CPU fraction. Exercised through the adapter's fake-socket tests
//! alongside the I/O it sits beside, as the rest of the adapter is.

use bollard::models::{
    ContainerStatsResponse, ContainerSummary, ContainerSummaryHealthStatusEnum,
    ContainerSummaryStateEnum, PortSummary,
};

use super::{PROJECT_LABEL, SERVICE_LABEL};
use crate::ports::docker::{Container, Health, Lifecycle, Published, Stats};

/// What the engine calls a container's state, in the port's vocabulary.
///
/// The two vocabularies agree because the port was written from this list; the
/// mapping exists so that a future engine adding a state cannot silently become
/// one lemonfiber already has a meaning for.
pub(super) const fn lifecycle(state: Option<ContainerSummaryStateEnum>) -> Lifecycle {
    match state {
        Some(ContainerSummaryStateEnum::CREATED) => Lifecycle::Created,
        Some(ContainerSummaryStateEnum::RUNNING) => Lifecycle::Running,
        Some(ContainerSummaryStateEnum::PAUSED) => Lifecycle::Paused,
        Some(ContainerSummaryStateEnum::RESTARTING) => Lifecycle::Restarting,
        Some(ContainerSummaryStateEnum::EXITED) => Lifecycle::Exited,
        Some(ContainerSummaryStateEnum::REMOVING | ContainerSummaryStateEnum::STOPPING) => {
            Lifecycle::Removing
        }
        // An engine that will not say counts as dead rather than as running:
        // the honest reading of silence is that nothing is known to be up.
        Some(ContainerSummaryStateEnum::DEAD | ContainerSummaryStateEnum::EMPTY) | None => {
            Lifecycle::Dead
        }
    }
}

/// What a container's own probe says, where it has one.
pub(super) const fn health(status: Option<ContainerSummaryHealthStatusEnum>) -> Health {
    match status {
        Some(ContainerSummaryHealthStatusEnum::STARTING) => Health::Starting,
        Some(ContainerSummaryHealthStatusEnum::HEALTHY) => Health::Healthy,
        Some(ContainerSummaryHealthStatusEnum::UNHEALTHY) => Health::Unhealthy,
        Some(ContainerSummaryHealthStatusEnum::NONE | ContainerSummaryHealthStatusEnum::EMPTY)
        | None => Health::None,
    }
}

/// The exit status a stopped container reports, where the engine still says.
///
/// The engine renders it into the human-readable status line — `Exited (137) 2
/// hours ago` — and offers it nowhere else in a container listing. Asking each
/// container for its full inspection instead would be one request per service
/// per refresh, which at a dashboard's rate is nineteen requests a second to
/// learn one number.
pub(super) fn exit_code(status: Option<&str>) -> Option<i32> {
    let text = status?;
    let open = text.find('(')?;
    let close = text.find(')')?;
    let inside = text.get(open + 1..close)?;
    inside.trim_start_matches("signal: ").parse().ok()
}

/// One container as the port describes them.
pub(super) fn describe(summary: ContainerSummary) -> Container {
    let labels = summary.labels.unwrap_or_default();
    Container {
        id: summary.id.unwrap_or_default(),
        project: labels.get(PROJECT_LABEL).cloned().unwrap_or_default(),
        service: labels.get(SERVICE_LABEL).cloned().unwrap_or_default(),
        lifecycle: lifecycle(summary.state),
        health: health(summary.health.and_then(|reported| reported.status)),
        published: published(summary.ports.unwrap_or_default()),
        exit: exit_code(summary.status.as_deref()),
    }
}

/// The host addresses a container actually answers on.
///
/// A port with no host port is one the container exposes and nothing published, so
/// there is no host address to have a policy about and it is left out. An address the
/// engine reports that is not one — which it has never been seen to do — is left out
/// for the same reason: what this list is for is holding real addresses to a rule, and
/// a word that is not an address is not a binding anybody can check.
fn published(ports: Vec<PortSummary>) -> Vec<Published> {
    ports
        .into_iter()
        .filter_map(|port| {
            Some(Published {
                address: port.ip?.parse().ok()?,
                port: port.public_port?,
            })
        })
        .collect()
}

/// Split a timestamped log line into when it was written and what it said.
///
/// The engine prefixes each line with an RFC 3339 instant when asked to, and
/// the separator is the first space. A line that does not have one is passed
/// through whole rather than guessed at — a log viewer showing half a message
/// as a timestamp is worse than one showing no timestamp.
pub(super) fn split_timestamp(line: &str) -> (Option<String>, String) {
    let Some((stamp, rest)) = line.split_once(' ') else {
        return (None, line.to_owned());
    };
    if stamp.len() >= 20 && stamp.contains('T') && stamp.ends_with('Z') {
        return (Some(stamp.to_owned()), rest.to_owned());
    }
    (None, line.to_owned())
}

/// Fraction of one core, from two samples of the same container.
///
/// The engine reports totals, not rates, so a rate is the difference between
/// consecutive samples. With no previous sample at all, or where two samples
/// fall in the same system-time tick, there is no interval to divide by and the
/// answer is zero rather than a division that would be infinite or invented.
pub(super) fn cpu_fraction(sample: &ContainerStatsResponse) -> f64 {
    // Both readings or neither. One without the other is not half an answer,
    // it is the same absence of one.
    let (Some(current), Some(previous)) = (sample.cpu_stats.as_ref(), sample.precpu_stats.as_ref())
    else {
        return 0.0;
    };

    let used = current
        .cpu_usage
        .as_ref()
        .and_then(|usage| usage.total_usage)
        .unwrap_or_default()
        .saturating_sub(
            previous
                .cpu_usage
                .as_ref()
                .and_then(|usage| usage.total_usage)
                .unwrap_or_default(),
        );
    let elapsed = current
        .system_cpu_usage
        .unwrap_or_default()
        .saturating_sub(previous.system_cpu_usage.unwrap_or_default());

    if elapsed == 0 {
        return 0.0;
    }

    // The ratio is taken in whole numbers and scaled down afterwards. Both
    // counters are nanoseconds and outgrow the range a float represents
    // exactly, and converting them directly is the kind of cast that is fine
    // until a container has been up for a month. Four decimal places is already
    // more precision than any reading of this number survives.
    let scaled = used.saturating_mul(SCALE) / elapsed;
    let cores = f64::from(current.online_cpus.unwrap_or(1).max(1));
    f64::from(u32::try_from(scaled).unwrap_or(u32::MAX)) / f64::from(SCALE_F) * cores
}

/// The whole-number scale the CPU ratio is computed at.
const SCALE: u64 = 10_000;

/// The same scale, in the width a float conversion accepts without loss.
const SCALE_F: u32 = 10_000;

/// Resource use for one container at one moment.
pub(super) fn sampled(sample: &ContainerStatsResponse) -> Stats {
    Stats {
        cpu: cpu_fraction(sample),
        memory_bytes: sample
            .memory_stats
            .as_ref()
            .and_then(|memory| memory.usage)
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use bollard::models::PortSummary;

    use super::published;

    /// One port as the engine reports it.
    fn reported(ip: Option<&str>, public: Option<u16>) -> PortSummary {
        PortSummary {
            ip: ip.map(str::to_owned),
            private_port: 8989,
            public_port: public,
            typ: None,
        }
    }

    /// What the engine says it published is what comes back, address by address.
    #[test]
    fn every_address_the_engine_reports_is_read_back() {
        let read = published(vec![
            reported(Some("127.0.0.1"), Some(8989)),
            reported(Some("::"), Some(8989)),
        ]);
        assert_eq!(read.len(), 2);
        assert!(read
            .first()
            .is_some_and(|first| first.address.is_loopback()));
        assert!(read
            .get(1)
            .is_some_and(|second| second.address.is_unspecified()));
        assert!(read.iter().all(|entry| entry.port == 8989));
    }

    /// A port with no host address and one with no host port are the same thing: a
    /// port the container exposes and nothing published, which has no binding for
    /// anybody to have a policy about.
    #[test]
    fn a_port_nothing_published_is_not_a_binding() {
        assert!(published(vec![reported(None, Some(8989))]).is_empty());
        assert!(published(vec![reported(Some("0.0.0.0"), None)]).is_empty());
    }

    /// A word the engine gives that is not an address is left out rather than
    /// guessed at, because what this list is for is holding real addresses to a rule.
    #[test]
    fn a_word_that_is_not_an_address_is_left_out() {
        assert!(published(vec![reported(Some("somewhere"), Some(8989))]).is_empty());
    }
}
