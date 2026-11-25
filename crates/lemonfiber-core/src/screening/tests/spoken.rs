//! A member's grant renewed as their client speaks.

use super::{ctx_over, server};
use crate::app::surfacing::Answer;
use crate::app::{dispatch, Command, Outcome, Surfacing};

/// A member whose client spoke has their grant renewed, until the day it runs out.
#[tokio::test]
async fn a_member_whose_client_spoke_holds_a_grant_until_it_runs_out() {
    let ctx = ctx_over(server(Vec::new()), "spoke");
    let renewed = dispatch(
        Command::Surface(Surfacing::Spoke {
            member: "a7f3".to_owned(),
        }),
        &ctx,
    )
    .await;
    let until = crate::screening::grants::until(ctx.today()).map(crate::screening::grants::written);
    assert!(
        matches!(renewed, Ok(Outcome::Surfaced(Answer::Renewed(said)))
        if !said.rehearsed && said.member == "a7f3" && Some(&said.lasts_until) == until.as_ref())
    );
    let kept = crate::app::targets::beside_env(&ctx, "grants.json")
        .and_then(|kept| std::fs::read_to_string(kept).ok())
        .unwrap_or_default();
    assert!(kept.contains("a7f3"), "{kept}");
}
