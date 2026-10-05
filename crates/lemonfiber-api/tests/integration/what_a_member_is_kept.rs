//! A member's household read, kept for a few seconds rather than read again at every
//! asking — and kept for that member alone.

use crate::door;
use door::*;
use lemonfiber_api::read::kept::{Kept, KEPT_FOR};
use lemonfiber_core::app::Command;
use lemonfiber_fixtures::http::Answer as Reply;
use lemonfiber_fixtures::ports::Following;

/// A stack whose media server signs this program in and holds nobody, on a clock that
/// moves with the runtime.
fn a_household(named: &str) -> (Ctx, Arc<Fake>) {
    let transport = Fake::by_path(vec![
        (
            "/Users/AuthenticateByName",
            Reply::reply(200, r#"{"AccessToken":"token"}"#),
        ),
        ("/Users", Reply::reply(200, "[]")),
    ]);
    let mut ctx = lemonfiber_testing::a_context()
        .runner(Arc::new(Idle))
        .clock(Following::started())
        .build()
        .with_http(transport.clone());
    ctx.settings.env_file = Some(a_directory(named).join(".env"));
    seeded(&ctx);
    (ctx, transport)
}

/// What a member's household read answers, as text.
async fn read(kept: &Kept, ctx: &Ctx, member: &str) -> String {
    let answer = kept
        .read(
            ctx,
            member,
            Command::Household {
                member: Some(member.to_owned()),
            },
        )
        .await;
    let Ok(body) = to_bytes(answer.into_body(), 1024 * 1024).await else {
        unreachable!("an answer this surface produces is one that can be read")
    };
    String::from_utf8_lossy(&body).into_owned()
}

#[tokio::test(start_paused = true)]
async fn a_members_household_is_read_once_and_handed_back_until_it_runs_out() {
    let (ctx, transport) = a_household("kept-member");
    let kept = Kept::default();

    let first = read(&kept, &ctx, "a7f3").await;
    let reached = transport.requests().len();
    assert!(reached > 0, "the first asking read nothing");

    let again = read(&kept, &ctx, "a7f3").await;
    assert_eq!(again, first);
    assert_eq!(
        transport.requests().len(),
        reached,
        "asking again at once read the household again"
    );

    tokio::time::advance(KEPT_FOR).await;
    let _ = read(&kept, &ctx, "a7f3").await;
    assert!(
        transport.requests().len() > reached,
        "a read that had run out was handed back"
    );
    let _ = fs::remove_dir_all(a_directory("kept-member"));
}

#[tokio::test(start_paused = true)]
async fn what_one_member_read_is_never_handed_to_another() {
    let (ctx, transport) = a_household("kept-apart");
    let kept = Kept::default();

    let _ = read(&kept, &ctx, "a7f3").await;
    let reached = transport.requests().len();
    let _ = read(&kept, &ctx, "b9c1").await;

    assert!(
        transport.requests().len() > reached,
        "a second member was handed the first member's read"
    );
    let _ = fs::remove_dir_all(a_directory("kept-apart"));
}

/// Keeping a member's read keeps nothing about whether they may ask: removed at the
/// media server, they are refused at their next call, kept read or not.
#[tokio::test]
async fn a_member_removed_is_refused_at_their_next_call_whatever_was_kept() {
    let household = AHousehold::knowing("a7f3");
    let (router, _, _) = door_with(
        Some(keeping("kept-removed")),
        Arc::clone(&household),
        not_the_token(),
    );
    let opened = asked(
        router.clone(),
        "POST",
        SESSION,
        &from_here(),
        &offering_as("ana", &hers()),
    )
    .await;
    let mut carried = from_here();
    carried.push((TOKEN_HEADER, session(&opened.body)));

    let before = asked(router.clone(), "GET", "/api/requests", &carried, "").await;
    household.withdraw();
    let after = asked(router, "GET", "/api/requests", &carried, "").await;

    assert_ne!(before.status, StatusCode::FORBIDDEN, "{}", before.body);
    assert_eq!(after.status, StatusCode::FORBIDDEN, "{}", after.body);
    let _ = fs::remove_dir_all(a_directory("kept-removed"));
}

/// A household that counts how often it is opened, and opens nothing until told to.
struct Counted {
    /// How many times it has been opened.
    opened: std::sync::atomic::AtomicUsize,
    /// Whether there is anything to open yet.
    there: AtomicBool,
    /// What it opens.
    household: Arc<dyn Household>,
}

impl HouseholdAtHand for Counted {
    fn now(&self) -> Option<Arc<dyn Household>> {
        self.opened.fetch_add(1, Ordering::SeqCst);
        self.there
            .load(Ordering::SeqCst)
            .then(|| Arc::clone(&self.household))
    }

    fn vouches_for(&self, id: &str) -> bool {
        id == "a7f3"
    }
}

#[tokio::test(start_paused = true)]
async fn an_opened_household_is_used_for_a_while_and_none_is_never_kept() {
    let counted = Arc::new(Counted {
        opened: std::sync::atomic::AtomicUsize::new(0),
        there: AtomicBool::new(false),
        household: AHousehold::knowing("a7f3") as Arc<dyn Household>,
    });
    let remembered = lemonfiber_api::admission::remembered::Remembered::over(
        Arc::clone(&counted) as Arc<dyn HouseholdAtHand>
    );
    let opened = || counted.opened.load(Ordering::SeqCst);

    assert!(remembered.now().is_none());
    assert!(remembered.now().is_none());
    assert_eq!(
        opened(),
        2,
        "a stack with no household was answered from memory"
    );

    counted.there.store(true, Ordering::SeqCst);
    assert!(remembered.now().is_some());
    assert!(remembered.now().is_some());
    assert_eq!(opened(), 3, "an opened household was opened again at once");

    tokio::time::advance(lemonfiber_api::admission::remembered::KEPT_FOR).await;
    assert!(remembered.now().is_some());
    assert_eq!(opened(), 4, "a household kept past its time was used");

    assert!(remembered.vouches_for("a7f3"));
    assert!(!remembered.vouches_for("b9c1"));
}
