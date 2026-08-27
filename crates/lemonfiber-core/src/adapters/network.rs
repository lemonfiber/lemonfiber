//! Asking the operating system what this machine is called.

use std::sync::Arc;

use async_trait::async_trait;

use crate::ports::network::Site;
use crate::ports::Runner;

/// The program that answers what this machine calls itself.
const HOSTNAME: &str = "hostname";

/// This machine, asked about itself.
///
/// Through the process port every other program here goes through rather than
/// through a seam of its own: a run that has already scripted what programs say has
/// scripted this one, and there stays one place in this workspace that spawns
/// anything.
pub struct Here {
    /// How `hostname` is run.
    runner: Arc<dyn Runner>,
}

impl Here {
    /// This machine, asked through the given runner.
    #[must_use]
    pub const fn over(runner: Arc<dyn Runner>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl Site for Here {
    async fn name(&self) -> Option<String> {
        let output = self.runner.run(&[HOSTNAME.to_owned()]).await.ok()?;
        if !output.succeeded() {
            return None;
        }
        let said = output.stdout.trim().trim_end_matches('.');
        if said.is_empty() {
            return None;
        }
        Some(said.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::{Here, HOSTNAME};
    use crate::ports::network::Site;
    use crate::ports::process::Output;
    use lemonfiber_fixtures::support::{Recording, Scripted};
    use std::sync::Arc;

    /// A runner answering every program with the given output.
    fn saying(stdout: &str) -> Arc<Scripted> {
        Arc::new(Scripted(Ok(Output {
            status: Some(0),
            stdout: stdout.to_owned(),
            stderr: String::new(),
        })))
    }

    #[tokio::test]
    async fn the_name_is_what_the_machine_says_it_is() {
        assert_eq!(
            Here::over(saying("kitchen-nas\n")).name().await,
            Some("kitchen-nas".to_owned())
        );
    }

    #[tokio::test]
    async fn a_qualified_name_keeps_every_part_but_its_trailing_dot() {
        assert_eq!(
            Here::over(saying("kitchen-nas.lan.\n")).name().await,
            Some("kitchen-nas.lan".to_owned())
        );
    }

    #[tokio::test]
    async fn a_machine_that_answers_with_nothing_has_no_name() {
        assert_eq!(Here::over(saying("   \n")).name().await, None);
        assert_eq!(Here::over(saying(".")).name().await, None);
    }

    #[tokio::test]
    async fn a_program_that_exits_badly_leaves_the_name_unknown() {
        let here = Here::over(Arc::new(Scripted(Ok(Output {
            status: Some(1),
            stdout: "kitchen-nas".to_owned(),
            stderr: String::new(),
        }))));
        assert_eq!(here.name().await, None);
    }

    #[tokio::test]
    async fn a_program_that_will_not_run_leaves_the_name_unknown() {
        let here = Here::over(Arc::new(Scripted(Err(
            crate::ports::process::Failure::NotFound {
                program: HOSTNAME.to_owned(),
            },
        ))));
        assert_eq!(here.name().await, None);
    }

    #[tokio::test]
    async fn the_name_is_asked_of_the_program_that_answers_it() {
        let asked = Arc::new(Recording::answering(Ok(Output {
            status: Some(0),
            stdout: "kitchen-nas".to_owned(),
            stderr: String::new(),
        })));
        let here = Here::over(Arc::clone(&asked) as Arc<dyn crate::ports::Runner>);
        assert_eq!(here.name().await, Some("kitchen-nas".to_owned()));
        assert!(asked.ran(HOSTNAME));
    }
}
