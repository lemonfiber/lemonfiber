//! The release is created with a token that may create one.
//!
//! `dist` generates `release.yml`, and what it generates gives the host job
//! `contents: write` and the ordinary `GITHUB_TOKEN`. That token cannot have write
//! here: the default workflow permission is `read` at both the organisation and the
//! repository, and a job may only ask for what the default allows. So the generated
//! file produces
//!
//! ```text
//! HTTP 403: Resource not accessible by integration
//! ```
//!
//! at the last step of a release, after every artefact has been built — which is
//! the worst place to find out, because the tag is already pushed and this
//! repository's tags are immutable.
//!
//! The fix is an App token, minted the way the record, the site and the contract
//! move already mint one. It is an edit to a generated file, so the next `dist
//! init` will drop it unless somebody notices. This is what notices.

use std::fs;

/// The workflow `dist` writes and this repository amends.
const RELEASE: &str = "../../.github/workflows/release.yml";

/// What the amendment is made of, each of which the release stops working without.
const MINTED: [(&str, &str); 4] = [
    (
        "actions/create-github-app-token",
        "the step that mints a token an App speaks with",
    ),
    (
        "client-id: ${{ vars.RELEASE_CLIENT_ID }}",
        "the App it is minted as",
    ),
    (
        "private-key: ${{ secrets.RELEASE_APP_KEY }}",
        "the key it is minted with",
    ),
    (
        "GH_TOKEN: ${{ steps.token.outputs.token }}",
        "the release being created with it rather than with the capped one",
    ),
];

#[test]
fn the_release_is_created_with_a_token_that_may_create_one() {
    let workflow = fs::read_to_string(RELEASE).unwrap_or_default();
    assert!(
        workflow.contains("Create GitHub Release"),
        "read no release workflow at {RELEASE}, so this is holding nothing to anything"
    );

    let missing: Vec<&str> = MINTED
        .iter()
        .filter(|(mark, _)| !workflow.contains(mark))
        .map(|(_, what)| *what)
        .collect();

    assert!(
        missing.is_empty(),
        "the release workflow has lost {missing:?} — `dist init` regenerates this file \
         and drops what is not its own. Without it the last step of a release answers \
         403 after every artefact has been built, and the tag it was cut for cannot be \
         moved."
    );
}
