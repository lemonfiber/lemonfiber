//! Running programs on this machine.

use async_trait::async_trait;
use tokio::process::Command;

use crate::ports::process::{Failure, Output, Runner};

/// Runs programs as child processes of this one.
#[derive(Debug, Default, Clone, Copy)]
pub struct Local;

#[async_trait]
impl Runner for Local {
    async fn run(&self, argv: &[String]) -> Result<Output, Failure> {
        let Some((program, arguments)) = argv.split_first() else {
            return Err(Failure::Unusable {
                program: String::new(),
                reason: "no program was given".to_owned(),
            });
        };

        let result = Command::new(program).args(arguments).output().await;
        let output = match result {
            Ok(output) => output,
            // The engine distinguishes "not installed" from "installed and
            // refusing", and so must the operator: one is a download, the other
            // is usually a permission or daemon problem.
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(Failure::NotFound {
                    program: program.clone(),
                })
            }
            Err(err) => {
                return Err(Failure::Unusable {
                    program: program.clone(),
                    reason: err.to_string(),
                })
            }
        };

        Ok(Output {
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Failure, Local, Runner};

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    #[tokio::test]
    async fn collects_what_a_program_wrote_and_how_it_exited() {
        let spoken = Local
            .run(&argv(&["echo", "hello"]))
            .await
            .map(|output| (output.succeeded(), output.stdout.trim().to_owned()))
            .ok();
        assert_eq!(spoken, Some((true, "hello".to_owned())));
    }

    #[tokio::test]
    async fn a_non_zero_exit_is_a_result_rather_than_an_error() {
        let outcome = Local
            .run(&argv(&["false"]))
            .await
            .map(|output| output.succeeded())
            .ok();
        assert_eq!(outcome, Some(false), "a failed program still ran");
    }

    #[tokio::test]
    async fn a_missing_program_is_distinguished_from_a_broken_one() {
        let result = Local
            .run(&argv(&["lemonfiber-no-such-program-exists"]))
            .await;
        assert!(matches!(result, Err(Failure::NotFound { .. })));
    }

    #[tokio::test]
    async fn an_empty_argument_vector_is_refused_rather_than_spawned() {
        assert!(matches!(
            Local.run(&[]).await,
            Err(Failure::Unusable { .. })
        ));
    }
}
