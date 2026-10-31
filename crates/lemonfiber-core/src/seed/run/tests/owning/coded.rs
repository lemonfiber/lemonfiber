//! Claiming a part against an account outside the house, with the code that account shows.

use std::path::{Path, PathBuf};

use super::*;
use crate::ports::asking::Asking;
use crate::test_support::Typing;

/// The code the part is claimed with.
const CODE: &str = "claim-code";

/// The seconds it holds once entered.
const LAPSES_S: u32 = 240;

/// When the first run in each test runs, in seconds into the day.
const FIRST: u64 = 1_000;

/// What the operator types.
const TYPED: &str = "4321";

/// The claimer, its adapter handed [`CODE`] alone, declared as a code that lapses.
fn coded() -> crate::plugin::Installed {
    let mut installed = contracted(PLUGIN, SERVICE, "owner.claim");
    for placed in &mut installed.services {
        placed.provides.clear();
        placed
            .upstream
            .insert("claim_code".to_owned(), CODE.to_owned());
    }
    installed.declared.secrets = vec![crate::plugin::Secret {
        id: CODE.to_owned(),
        of: SERVICE.to_owned(),
        why: "The code the account shows".to_owned(),
        minted: false,
        serves: None,
        was: None,
        needs: lemonfiber_manifest::container::choices::Needs::new(),
        needed_to_start: false,
        lapses_s: Some(LAPSES_S),
    }];
    installed
}

/// The stack beside [`coded`].
fn coded_beside(project: &Path) -> crate::wiring::Fillers {
    fillers_trusting(Vec::new(), &[coded()], project, &first_party(PLUGIN))
}

/// A context reaching the claimer over `http` at `seconds` into the day, its stack in
/// `project`, asking through `asking`.
fn coded_ctx(project: &Path, http: Arc<Fake>, seconds: u64, asking: Arc<dyn Asking>) -> Ctx {
    let mut ctx = contracted_ctx(project, SERVICE, true, http).with_asking(asking);
    ctx.seams.clock = lemonfiber_fixtures::ports::Stopped::into_today(seconds);
    ctx
}

/// The setting the code is kept under.
fn setting() -> String {
    crate::settings::named(PLUGIN, CODE)
}

/// What the secret store holds for the code.
fn kept(ctx: &Ctx) -> Option<String> {
    crate::settings::secrets_at(ctx)
        .and_then(|at| crate::settings::Secrets::at(&at).ok())
        .and_then(|secrets| secrets.value(&setting()).map(str::to_owned))
}

/// Where the adapter reads its upstream's credentials.
fn upstream_json(ctx: &Ctx) -> PathBuf {
    crate::plugin::configuration(
        &crate::app::project::directory(ctx).unwrap_or_default(),
        SERVICE,
    )
    .join(lemonfiber_contract::adapter::UPSTREAM_FILE)
}

/// What the adapter's `upstream.json` says.
fn handed(ctx: &Ctx) -> String {
    std::fs::read_to_string(upstream_json(ctx)).unwrap_or_default()
}

/// The code held as entered `seconds` into the day.
fn entered(ctx: &Ctx, seconds: u64) {
    let at = lemonfiber_fixtures::ports::Stopped::into_today(seconds);
    let at = crate::ports::time::Clock::now(at.as_ref())
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    let _ = crate::settings::lapsing(
        &crate::settings::secrets_at(ctx).unwrap_or_default(),
        &setting(),
        TYPED,
        at,
    );
}

/// How the part stands after one seed.
async fn seeded(ctx: &Ctx, project: &Path) -> Vec<crate::seed::Wiring> {
    super::super::super::owning::owned(
        ctx,
        &coded_beside(project),
        &[coded()],
        &mut Baseline::new(),
    )
    .await
}

#[tokio::test]
async fn an_open_part_is_claimed_with_the_code_the_operator_enters_and_the_code_is_then_dropped() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-coded-claimed");
    let http = Fake::by_path(vec![(OWNER, holding("open")), (CLAIM, holding("ours"))]);
    let typing = Typing::answering(TYPED);
    let ctx = coded_ctx(&project, http.clone(), FIRST, typing.clone());

    let wirings = seeded(&ctx, &project).await;

    assert_eq!(
        wirings.first().map(|one| &one.state),
        Some(&crate::seed::State::Wired),
        "{wirings:?}"
    );
    let asked = typing.asked();
    assert!(
        matches!(asked.as_slice(), [(question, true)] if question.contains(&setting())),
        "{asked:?}"
    );
    assert_eq!(asked_at(&http, CLAIM), 1);
    assert_eq!(kept(&ctx), None);
    assert_eq!(handed(&ctx), "{}");
}

#[tokio::test]
async fn a_claim_left_unanswered_keeps_the_code_until_it_lapses_and_is_then_dropped() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-coded-lapsed");
    let http = Fake::by_path(vec![
        (OWNER, holding("open")),
        (CLAIM, Answer::reply(500, "")),
    ]);
    let ctx = coded_ctx(&project, http.clone(), FIRST, Typing::answering(TYPED));

    seeded(&ctx, &project).await;

    assert_eq!(kept(&ctx).as_deref(), Some(TYPED));
    assert!(handed(&ctx).contains(TYPED), "{}", handed(&ctx));

    let mut later = ctx
        .clone()
        .with_asking(Arc::new(crate::ports::asking::Nobody));
    later.seams.clock =
        lemonfiber_fixtures::ports::Stopped::into_today(FIRST + u64::from(LAPSES_S));
    seeded(&later, &project).await;

    assert_eq!(kept(&later), None);
    assert_eq!(handed(&later), "{}");
}

#[tokio::test]
async fn a_code_entered_longer_ago_than_it_holds_is_asked_for_afresh() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-coded-afresh");
    let http = Fake::by_path(vec![(OWNER, holding("open")), (CLAIM, holding("ours"))]);
    let typing = Typing::answering(TYPED);
    let ctx = coded_ctx(
        &project,
        http.clone(),
        FIRST + u64::from(LAPSES_S),
        typing.clone(),
    );
    entered(&ctx, FIRST);

    let wirings = seeded(&ctx, &project).await;

    assert_eq!(
        wirings.first().map(|one| &one.state),
        Some(&crate::seed::State::Wired),
        "{wirings:?}"
    );
    assert_eq!(typing.asked().len(), 1);
    assert_eq!(kept(&ctx), None);
}

#[tokio::test]
async fn an_open_part_nobody_enters_a_code_for_is_left_open_naming_the_code() {
    for (name, asking) in [
        (
            "nobody",
            Arc::new(crate::ports::asking::Nobody) as Arc<dyn Asking>,
        ),
        ("nothing", Typing::answering("") as Arc<dyn Asking>),
    ] {
        let project = lemonfiber_fixtures::scratch::Scratch::new(&format!("owning-coded-{name}"));
        let http = Fake::by_path(vec![(OWNER, holding("open")), (CLAIM, holding("ours"))]);
        let ctx = coded_ctx(&project, http.clone(), FIRST, asking);

        let wirings = seeded(&ctx, &project).await;

        assert!(
            wirings.first().is_some_and(|one| matches!(&one.state,
                crate::seed::State::Skipped { reason } if reason.contains(&setting()))
                && matches!(&one.severity, crate::seed::Severity::Warning { .. })),
            "{name}: {wirings:?}"
        );
        assert_eq!(asked_at(&http, CLAIM), 0, "{name}");
        assert_eq!(kept(&ctx), None, "{name}");
    }
}

#[tokio::test]
async fn a_part_already_held_is_asked_for_no_code() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-coded-held");
    let http = Fake::by_path(vec![(OWNER, holding("ours")), (CLAIM, holding("ours"))]);
    let typing = Typing::answering(TYPED);
    let ctx = coded_ctx(&project, http.clone(), FIRST, typing.clone());

    let wirings = seeded(&ctx, &project).await;

    assert_eq!(
        wirings.first().map(|one| &one.state),
        Some(&crate::seed::State::Wired),
        "{wirings:?}"
    );
    assert!(typing.asked().is_empty());
    assert_eq!(asked_at(&http, CLAIM), 0);
}

#[tokio::test]
async fn a_code_entered_within_its_seconds_is_claimed_with_and_never_asked_again() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-coded-fresh");
    let http = Fake::by_path(vec![(OWNER, holding("open")), (CLAIM, holding("ours"))]);
    let typing = Typing::answering("other");
    let ctx = coded_ctx(&project, http.clone(), FIRST + 10, typing.clone());
    entered(&ctx, FIRST);

    let wirings = seeded(&ctx, &project).await;

    assert_eq!(
        wirings.first().map(|one| &one.state),
        Some(&crate::seed::State::Wired),
        "{wirings:?}"
    );
    assert!(typing.asked().is_empty());
    assert_eq!(kept(&ctx), None);
}

#[tokio::test]
async fn a_reset_claims_back_an_open_part_with_the_code_the_operator_enters() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-coded-reset");
    let http = Fake::by_path(vec![(OWNER, holding("open")), (CLAIM, holding("ours"))]);
    let typing = Typing::answering(TYPED);
    let ctx = coded_ctx(&project, http.clone(), FIRST, typing.clone());

    let wirings = super::super::super::owning::put_back(
        &ctx,
        &coded_beside(&project),
        &[coded()],
        &recorded(),
        true,
    )
    .await;

    assert_eq!(
        wirings.first().map(|one| &one.state),
        Some(&crate::seed::State::Wired),
        "{wirings:?}"
    );
    assert_eq!(typing.asked().len(), 1);
    assert_eq!(kept(&ctx), None);
}

/// What breaks a context's keeping or handing over a code.
type Breaking = fn(&mut Ctx);

#[tokio::test]
async fn a_code_that_cannot_be_kept_or_handed_over_fails_the_claim_before_it_is_asked() {
    let unstored: Breaking = |ctx| ctx.settings.env_file = None;
    let stackless: Breaking = |ctx| ctx.stack = crate::test_support::carried_stack();
    for (name, typed, broken) in [
        ("unstored", TYPED, unstored),
        ("stackless", TYPED, stackless),
        ("spanning", "43\n21", |_: &mut Ctx| {}),
    ] {
        let project = lemonfiber_fixtures::scratch::Scratch::new(&format!("owning-coded-{name}"));
        let http = Fake::by_path(vec![(OWNER, holding("open")), (CLAIM, holding("ours"))]);
        let mut ctx = coded_ctx(&project, http.clone(), FIRST, Typing::answering(typed));
        broken(&mut ctx);

        let wirings = seeded(&ctx, &project).await;

        assert!(
            wirings
                .first()
                .is_some_and(|one| matches!(one.state, crate::seed::State::Failed { .. })),
            "{name}: {wirings:?}"
        );
        assert_eq!(asked_at(&http, CLAIM), 0, "{name}");
    }
}

#[tokio::test]
async fn a_code_whose_upstream_cannot_be_written_again_fails_lapsing_and_dropping_it() {
    for (name, seconds) in [
        ("lapsing", FIRST + u64::from(LAPSES_S)),
        ("dropping", FIRST + 10),
    ] {
        let project = lemonfiber_fixtures::scratch::Scratch::new(&format!("owning-coded-{name}"));
        let http = Fake::by_path(vec![(OWNER, holding("open")), (CLAIM, holding("ours"))]);
        let ctx = coded_ctx(&project, http.clone(), seconds, Typing::answering(TYPED));
        entered(&ctx, FIRST);
        let _ = std::fs::create_dir_all(upstream_json(&ctx));

        let wirings = seeded(&ctx, &project).await;

        assert!(
            wirings
                .first()
                .is_some_and(|one| matches!(one.state, crate::seed::State::Failed { .. })),
            "{name}: {wirings:?}"
        );
        assert_eq!(
            asked_at(&http, CLAIM),
            usize::from(name == "dropping"),
            "{name}"
        );
    }
}

#[tokio::test]
async fn a_rehearsal_leaves_a_code_that_lapsed_where_it_is() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-coded-rehearsed");
    let http = Fake::by_path(vec![(OWNER, holding("open")), (CLAIM, holding("ours"))]);
    let typing = Typing::answering(TYPED);
    let mut ctx = coded_ctx(
        &project,
        http.clone(),
        FIRST + u64::from(LAPSES_S),
        typing.clone(),
    );
    ctx.dry_run = true;
    entered(&ctx, FIRST);

    seeded(&ctx, &project).await;

    assert_eq!(kept(&ctx).as_deref(), Some(TYPED));
    assert!(typing.asked().is_empty());
    assert_eq!(asked_at(&http, CLAIM), 0);
}

/// A part answering who holds it as `holder`, and a claim as `claimed`, counting claims.
struct Part {
    holder: crate::ports::service::Owner,
    claimed: crate::ports::service::Owner,
    claims: std::sync::atomic::AtomicUsize,
    blocking: Option<PathBuf>,
}

#[async_trait::async_trait]
impl crate::ports::service::Ownership for Part {
    async fn owner(&self) -> Result<crate::ports::service::Owner, crate::ports::service::Failure> {
        Ok(self.holder)
    }

    async fn claim(&self) -> Result<crate::ports::service::Owner, crate::ports::service::Failure> {
        self.claims
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if let Some(blocking) = &self.blocking {
            let _ = std::fs::remove_file(blocking);
            let _ = std::fs::create_dir_all(blocking);
        }
        Ok(self.claimed)
    }
}

/// A part held by `holder` that a claim leaves `claimed`.
fn part(holder: crate::ports::service::Owner, claimed: crate::ports::service::Owner) -> Part {
    Part {
        holder,
        claimed,
        claims: std::sync::atomic::AtomicUsize::new(0),
        blocking: None,
    }
}

/// What install's claim of `part`, of `installed`, comes to through `ctx`: who holds it, why
/// it could not be asked, or what is said where nobody entered its code.
async fn as_installed(
    ctx: &Ctx,
    installed: &crate::plugin::Installed,
    part: &Part,
) -> Result<Result<crate::ports::service::Owner, String>, String> {
    use super::super::super::owning::{claimed_as_installed, Installing};
    let Some(placed) = installed.services.first() else {
        return Err("no service".to_owned());
    };
    match claimed_as_installed(ctx, installed, placed, part).await {
        Installing::Answered(answered) => Ok(answered.map_err(|failure| failure.to_string())),
        Installing::Unentered(said) => Err(said),
    }
}

/// What install's claim of `part` comes to, asking through `asking`, its stack in
/// `project`.
async fn installed_claim(
    project: &Path,
    part: &Part,
    asking: Arc<dyn Asking>,
) -> (
    Ctx,
    Result<Result<crate::ports::service::Owner, String>, String>,
) {
    let ctx = coded_ctx(project, Fake::silent(), FIRST, asking);
    let claimed = as_installed(&ctx, &coded(), part).await;
    (ctx, claimed)
}

#[tokio::test]
async fn install_claims_an_open_part_with_the_code_entered_and_drops_it_after() {
    use crate::ports::service::Owner;
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-coded-install");
    let open = part(Owner::Open, Owner::Ours);
    let (ctx, claimed) = installed_claim(&project, &open, Typing::answering(TYPED)).await;
    assert!(matches!(claimed, Ok(Ok(Owner::Ours))), "{claimed:?}");
    assert_eq!(open.claims.load(std::sync::atomic::Ordering::Relaxed), 1);
    assert_eq!(kept(&ctx), None);
}

#[tokio::test]
async fn install_says_an_open_part_nobody_enters_a_code_for_is_open_naming_the_code() {
    use crate::ports::service::Owner;
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-coded-install-none");
    let open = part(Owner::Open, Owner::Ours);
    let (_, claimed) =
        installed_claim(&project, &open, Arc::new(crate::ports::asking::Nobody)).await;
    assert!(
        matches!(&claimed, Err(said) if said.contains(&setting())),
        "{claimed:?}"
    );
    assert_eq!(open.claims.load(std::sync::atomic::Ordering::Relaxed), 0);
}

#[tokio::test]
async fn install_asks_no_code_for_a_part_already_held_and_fails_one_it_cannot_keep() {
    use crate::ports::service::Owner;
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-coded-install-held");
    let theirs = part(Owner::Theirs, Owner::Ours);
    let typing = Typing::answering(TYPED);
    let (_, claimed) = installed_claim(&project, &theirs, typing.clone()).await;
    assert!(matches!(claimed, Ok(Ok(Owner::Theirs))), "{claimed:?}");
    assert!(typing.asked().is_empty());
    let spanning = part(Owner::Open, Owner::Ours);
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-coded-install-spanning");
    let (_, claimed) = installed_claim(&project, &spanning, Typing::answering("43\n21")).await;
    assert!(matches!(claimed, Ok(Err(_))), "{claimed:?}");
    assert_eq!(
        spanning.claims.load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[tokio::test]
async fn install_claims_a_part_that_takes_no_code_as_it_always_has() {
    use crate::ports::service::Owner;
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-coded-install-uncoded");
    let ctx = coded_ctx(&project, Fake::silent(), FIRST, Typing::answering(TYPED));
    let open = part(Owner::Open, Owner::Ours);
    let claimed = as_installed(&ctx, &claimer(), &open).await;
    assert!(matches!(claimed, Ok(Ok(Owner::Ours))), "{claimed:?}");
}

#[tokio::test]
async fn a_claim_brings_the_part_onto_the_household_and_finding_it_open_again_takes_it_off() {
    use super::super::super::owning::{kept_off, owned};
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-coded-kept-off");
    let fillers = coded_beside(&project);
    let off = |baseline: &Baseline| kept_off(&fillers, &[coded()], baseline);
    let mut baseline = Baseline::new();
    assert_eq!(off(&baseline), [SERVICE.to_owned()].into());

    let http = Fake::by_path(vec![(OWNER, holding("open")), (CLAIM, holding("ours"))]);
    let ctx = coded_ctx(&project, http, FIRST, Typing::answering(TYPED));
    owned(&ctx, &fillers, &[coded()], &mut baseline).await;
    assert!(off(&baseline).is_empty(), "{:?}", off(&baseline));

    let reopened = Fake::by_path(vec![(OWNER, holding("open")), (CLAIM, holding("ours"))]);
    let later = coded_ctx(
        &project,
        reopened.clone(),
        FIRST + 10,
        Typing::answering(TYPED),
    );
    let wirings = owned(&later, &fillers, &[coded()], &mut baseline).await;
    assert_eq!(
        wirings.first().map(|one| &one.state),
        Some(&crate::seed::State::Drifted),
        "{wirings:?}"
    );
    assert_eq!(off(&baseline), [SERVICE.to_owned()].into());
    assert_eq!(asked_at(&reopened, CLAIM), 0);
}

#[test]
fn a_part_that_takes_no_code_is_never_kept_off_the_household() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-uncoded-kept-off");
    let kept = super::super::super::owning::kept_off(
        &beside(&project, &first_party(PLUGIN)),
        &[claimer()],
        &Baseline::new(),
    );
    assert!(kept.is_empty(), "{kept:?}");
}

#[tokio::test]
async fn install_says_a_code_it_could_not_drop_after_the_claim_answered() {
    use crate::ports::service::Owner;
    let project = lemonfiber_fixtures::scratch::Scratch::new("owning-coded-install-undropped");
    let ctx = coded_ctx(&project, Fake::silent(), FIRST, Typing::answering(TYPED));
    let blocked = Part {
        blocking: Some(upstream_json(&ctx)),
        ..part(Owner::Open, Owner::Ours)
    };
    let claimed = as_installed(&ctx, &coded(), &blocked).await;
    assert!(matches!(claimed, Ok(Err(_))), "{claimed:?}");
    assert_eq!(blocked.claims.load(std::sync::atomic::Ordering::Relaxed), 1);
}
