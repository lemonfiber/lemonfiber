//! A run-and-exit service: one that runs to completion at the times its setting names,
//! and that nothing reaches while it is not running.

use lemonfiber_manifest::schedule::Cron;

use crate::schema::Manifest;
use crate::Violation;

/// Every `schedule` naming no time, a setting outside the plugin's namespace, or a
/// service something reaches: one that speaks a contract, publishes a port, declares a
/// tier or provides a capability.
pub(super) fn scheduled(manifest: &Manifest, found: &mut Vec<Violation>) {
    for (index, service) in manifest.services.iter().enumerate() {
        let Some(schedule) = &service.schedule else {
            continue;
        };
        let at = format!("service[{index}].schedule");
        let mut refuse = |field: &str, message: String| {
            found.push(Violation {
                location: format!("{at}{field}"),
                message,
            });
        };
        if Cron::parse(&schedule.default).is_none() {
            refuse(
                ".default",
                format!(
                    "{} is no time {} runs at: a five-field cron expression or one of @hourly, \
                     @daily, @weekly and @monthly",
                    schedule.default, service.id
                ),
            );
        }
        if !crate::claiming::namespaced_with(&schedule.setting, &manifest.plugin.id) {
            refuse(
                ".setting",
                format!(
                    "{} is not a setting of this plugin's, written {}:<name>, so the times {} \
                     runs at would be read from a setting another plugin or the core holds",
                    schedule.setting, manifest.plugin.id, service.id
                ),
            );
        }
        let reached = [
            (!service.speaks.is_empty(), "speaks a contract"),
            (
                service.port.is_some() || !service.publish.is_empty(),
                "publishes a port",
            ),
            (service.bind.is_some(), "declares bind"),
            (
                !service.provides.is_empty(),
                "provides a capability an ask is filled by",
            ),
        ];
        for (_, why) in reached.iter().filter(|(holds, _)| *holds) {
            refuse(
                "",
                format!(
                    "{} {why}, and a service that runs only at the times its schedule names is \
                     not there to be reached the rest of the time",
                    service.id
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests;
