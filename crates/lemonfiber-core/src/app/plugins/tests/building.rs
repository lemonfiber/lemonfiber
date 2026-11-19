//! What installing asks of an adapter in front of an upstream built on another's image.

use super::proving::verdicts;
use super::speaking::{adapting, speaking, UPSTREAM};
use super::*;

#[tokio::test]
async fn an_adapter_in_front_of_an_upstream_built_on_another_lists_the_release_it_was_built_from() {
    for (name, release, came) in [
        (
            "adapter-built-on-its-release",
            "1.11.0",
            ["passed", "passed"],
        ),
        (
            "adapter-built-on-another-release",
            "1.12.0",
            ["passed", "failed"],
        ),
    ] {
        let ctx = adapting(
            name,
            &[("komga-adapter", "127.0.0.1", 8080)],
            r#"["media.serve@1"]"#,
        );
        let built = speaking().replacen(
            "tag         = \"1.11.0\"\n",
            &format!(
                "tag         = \"1.11.0\"\nbuilt_on    = {{ image = \"gotson/komga\", release = \"{release}\" }}\n"
            ),
            1,
        );
        let said = verdicts(installing(&ctx, &source(name, &built)).await);
        assert_eq!(
            said.iter()
                .map(|verdict| came_to(verdict.as_ref()))
                .collect::<Vec<_>>(),
            came,
            "{name}: {said:?}"
        );
        let fault = format!(
            "it lists release 1.11.0 at {UPSTREAM}, and komga is built on gotson/komga {release}"
        );
        assert_eq!(
            said.iter().flatten().any(|verdict| matches!(
                verdict,
                Verdict::Failed { faults } if faults.contains(&fault)
            )),
            release != "1.11.0",
            "{name}: {said:?}"
        );
    }
}
