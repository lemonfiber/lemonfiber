//! How far the stack got towards being askable.
//!
//! A ladder rather than a handful of booleans, so the states are exclusive by
//! construction: a machine is at exactly one rung, and there is no way to describe
//! one that is both unconfigured and running.

use serde::{Deserialize, Serialize};

use crate::docker::{condition, Condition, Service, State};

/// How far the stack got towards being askable, which is not the same question as
/// whether anything is wrong with it.
///
/// One ladder for the whole crate. The dashboard's standing and the health summary
/// both read from it, because a surface that decided the engine was unreachable and
/// a summary that decided the stack was fine would be describing the same machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reach {
    /// Nothing has been set up yet.
    Unconfigured,
    /// Set up, and deliberately not running.
    Stopped,
    /// Coming up. Not yet a verdict on anything.
    Starting,
    /// Up, and answering.
    Running,
    /// Should be up; could not be asked.
    Unreachable,
}

impl Reach {
    /// Which rung a machine is on, from whether it is configured and what the
    /// engine said about its services.
    ///
    /// `None` for the services is an engine that could not be asked — distinct
    /// from an empty list, which is an engine that answered and reported nothing.
    /// Read here rather than at each surface, so a dashboard and a status command
    /// cannot place the same machine on two different rungs.
    #[must_use]
    pub fn of(configured: bool, services: Option<&[Service]>) -> Self {
        if !configured {
            return Self::Unconfigured;
        }
        let Some(services) = services else {
            return Self::Unreachable;
        };
        if condition(services) == Condition::Inactive {
            return Self::Stopped;
        }
        // Still inside its probe's start period, with nothing yet failed: coming up,
        // which is not a verdict on anything.
        if services
            .iter()
            .any(|service| service.state == State::Starting)
            && !services
                .iter()
                .any(|service| service.state.wants_attention())
        {
            return Self::Starting;
        }
        Self::Running
    }
}

#[cfg(test)]
mod tests {
    use super::Reach;
    use crate::docker::{Service, State};
    use lemonfiber_manifest::Criticality;

    /// One service in a state, at core criticality.
    fn service(id: &str, state: State) -> Service {
        Service {
            id: id.to_owned(),
            name: id.to_owned(),
            profile: "media".to_owned(),
            state,
            criticality: Criticality::Core,
            exit: None,
            depends_on: Vec::new(),
        }
    }

    #[test]
    fn a_machine_with_nothing_configured_is_on_the_bottom_rung() {
        assert_eq!(Reach::of(false, None), Reach::Unconfigured);
        // Configured outranks nothing else: even a running stack reads unconfigured
        // while there is no configuration to run it against.
        assert_eq!(
            Reach::of(false, Some(&[service("sonarr", State::Healthy)])),
            Reach::Unconfigured
        );
    }

    #[test]
    fn an_engine_that_could_not_be_asked_is_unreachable_not_stopped() {
        // The distinction the whole ladder exists for: nobody looked, which is not
        // the same as looking and finding nothing running.
        assert_eq!(Reach::of(true, None), Reach::Unreachable);
        assert_eq!(Reach::of(true, Some(&[])), Reach::Stopped);
    }

    #[test]
    fn a_stack_whose_containers_are_all_down_is_stopped() {
        let services = [
            service("sonarr", State::Stopped),
            service("radarr", State::Absent),
        ];
        assert_eq!(Reach::of(true, Some(&services)), Reach::Stopped);
    }

    #[test]
    fn a_stack_inside_its_start_period_is_starting() {
        let services = [
            service("sonarr", State::Starting),
            service("radarr", State::Healthy),
        ];
        assert_eq!(Reach::of(true, Some(&services)), Reach::Starting);
    }

    #[test]
    fn something_already_failed_is_running_rather_than_starting() {
        // Otherwise a stack with one container in a crash loop and another still
        // coming up would report "starting" indefinitely, and the failure with it.
        let services = [
            service("sonarr", State::Starting),
            service("radarr", State::CrashLooping),
        ];
        assert_eq!(Reach::of(true, Some(&services)), Reach::Running);
    }

    #[test]
    fn a_stack_that_is_up_is_running() {
        let services = [service("sonarr", State::Healthy)];
        assert_eq!(Reach::of(true, Some(&services)), Reach::Running);
    }

    #[test]
    fn the_rungs_are_distinct() {
        // The ladder exists to make the states exclusive; two rungs that compared
        // equal would let a machine be described as two things at once.
        let rungs = [
            Reach::Unconfigured,
            Reach::Stopped,
            Reach::Starting,
            Reach::Running,
            Reach::Unreachable,
        ];
        for (index, rung) in rungs.iter().enumerate() {
            for (other, another) in rungs.iter().enumerate() {
                assert_eq!(rung == another, index == other, "{rung:?} vs {another:?}");
            }
        }
    }
}
