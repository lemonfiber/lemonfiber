//! Choosing what the household may ask for.
//!
//! The half of the errand that writes. What the household *has* asked for, and what each
//! member has left, are read next door in [`super::household`] and answered there — and
//! this answers with that same reading, because what an operator wants to see after
//! changing a limit is the limit, on the people it applies to. A report of its own would
//! be a second description of the household able to disagree with the first.
//!
//! **Nothing named is nothing changed.** A run that gave a limit and no policy said
//! nothing about the policy, and the one in force stays. That is why the current setting
//! is read before anything is written: the two halves are one setting on the service, and
//! writing one from a value nobody chose would answer a request nobody made.
//!
//! **A per-person choice never touches the household's.** Setting somebody's own limit
//! writes against their account and leaves the default where it is, so a household that
//! trusts everybody and holds one person to five a week is one arrangement rather than
//! two settings fighting.

mod deciding;

pub(super) use deciding::deciding;

use crate::asking::Policy;
use crate::error::{Diagnose, Problem};
use crate::model::HouseholdReport;
use crate::ports::service::{
    Approving as _, Asking, Headroom, Household as _, Member, Quota, Requests as _,
};

use super::command::Chosen;
use super::targets::{jellyfin_reader, HouseholdAccess};
use super::Ctx;

/// Choose the policy, the limit, or both — for the household or for one person.
///
/// Answers with the household as it now stands, so what was chosen is read back off the
/// service rather than reported from what was sent.
pub(super) async fn allowing(ctx: &Ctx, chosen: &Chosen) -> Result<HouseholdReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let access = reached(ctx, &manifest.services).await?;

    // Each half reads what *it* is about to change before it changes it. A per-person
    // choice read against the household's own setting would inherit the household's
    // limit onto somebody who had one of their own, which is the opposite of leaving
    // alone what nobody named.
    let said = match chosen.member.as_deref() {
        Some(name) => one_person(ctx, &access, &manifest.services, name, chosen).await?,
        None => everybody(ctx, &access, chosen).await?,
    };

    let mut report = super::household::household(ctx, None).await?;
    report.findings.insert(0, said);
    Ok(report)
}

/// What is said where the service could not be asked what it holds.
const NOTHING_SET: &str = "what the household may ask for was not changed";

/// The request service, signed in, or the refusal that nothing was changed.
///
/// Its own step because both halves of this module begin with it, and because the
/// reading next door treats the same failure as a gap in a report rather than as a
/// refusal — a household that cannot be read is still worth listing, and a limit that
/// could not be written is not worth pretending was.
async fn reached(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> Result<HouseholdAccess, Box<Problem>> {
    super::household::reaching(ctx, services)
        .await
        .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_SET)))
}

/// What the household is to be left holding, from what was chosen and what it holds.
///
/// Trusting everybody lifts the limit rather than leaving one counting in the
/// background: a household told nothing is limited and a service still counting is two
/// answers to one question, and the one the operator reads is the wrong one.
fn settled(chosen: &Chosen, held: &Asking) -> Result<Asking, Box<Problem>> {
    let policy = chosen.policy.unwrap_or_else(|| Policy::of(held));
    let quota = if matches!(policy, Policy::Trusted) {
        None
    } else {
        chosen.quota.or(held.quota)
    };
    if policy.needs_a_limit() && quota.is_none() {
        return Err(Box::new(crate::asking::no_limit_named()));
    }
    Ok(Asking {
        approves_own: policy.arrives_unseen(),
        quota,
    })
}

/// Write the choice against the household, so it holds for everybody nobody chose
/// otherwise for.
async fn everybody(
    ctx: &Ctx,
    access: &HouseholdAccess,
    chosen: &Chosen,
) -> Result<String, Box<Problem>> {
    let held = access
        .seerr
        .asking()
        .await
        .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_SET)))?;
    let wanted = settled(chosen, &held)?;
    let said = format!("the household {}", now_reads(&wanted));
    if ctx.dry_run {
        return Ok(format!("{said} — rehearsed, and nothing was written"));
    }
    access
        .seerr
        .set_asking(&wanted)
        .await
        .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_SET)))?;
    Ok(said)
}

/// Write the choice against one person, leaving the household's own where it is.
///
/// **What is read first is theirs, not the household's.** Somebody already held to
/// three a week who is moved to waiting for approval keeps three a week; read against
/// the household's setting they would silently inherit whatever the house allows.
async fn one_person(
    ctx: &Ctx,
    access: &HouseholdAccess,
    services: &[lemonfiber_manifest::Service],
    name: &str,
    chosen: &Chosen,
) -> Result<String, Box<Problem>> {
    let account = found(ctx, services, name).await?;
    let held = access
        .seerr
        .requesting(&account.id)
        .await
        .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_SET)))?;
    let Some(held) = held else {
        return Err(Box::new(crate::asking::never_asked_here(&account.name)));
    };
    let headroom = access
        .seerr
        .left(&held.id)
        .await
        .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_SET)))?;
    let wanted = settled(chosen, &theirs(held.approves_own, headroom))?;
    let said = format!("{} {}", account.name, now_reads(&wanted));
    if ctx.dry_run {
        return Ok(format!("{said} — rehearsed, and nothing was written"));
    }
    // The limit first and the approval second. Between the two writes a member is held
    // to the new limit under the old policy, which is the harmless order: the other way
    // round would let requests through unseen against a limit not yet in force.
    access
        .seerr
        .set_quota(&held.id, wanted.quota)
        .await
        .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_SET)))?;
    access
        .seerr
        .approves_own(&held.id, wanted.approves_own)
        .await
        .map_err(|_| Box::new(crate::asking::unreachable(NOTHING_SET)))?;
    Ok(said)
}

/// What one member is under now, from what the service says about them.
///
/// The counts come back per kind and a household chooses one figure, so the one that
/// is set is the one read back — the same reading the whole-household setting gets,
/// and for the same reason: both halves are written together.
fn theirs(approves_own: bool, headroom: Headroom) -> Asking {
    let held = |left: crate::ports::service::Left| {
        left.limit
            .zip(left.days)
            .map(|(requests, days)| Quota { requests, days })
    };
    Asking {
        approves_own,
        quota: held(headroom.films).or_else(|| held(headroom.television)),
    }
}

/// The member the name was meant for, matched the forgiving way a name is typed.
///
/// The media server is asked who is here, because that is where the household is: a
/// name matched against the request service's own list would miss everybody who has an
/// account and has never opened it.
async fn found(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    name: &str,
) -> Result<Member, Box<Problem>> {
    let Some(server) = jellyfin_reader(ctx, services) else {
        return Err(Box::new(crate::asking::unreachable(NOTHING_SET)));
    };
    let Ok(accounts) = server.household().await else {
        return Err(Box::new(crate::asking::unreachable(NOTHING_SET)));
    };
    let wanted = name.trim().to_lowercase();
    accounts
        .iter()
        .find(|account| account.name.to_lowercase().contains(&wanted))
        .cloned()
        .ok_or_else(|| {
            let there: Vec<String> = accounts.iter().map(|held| held.name.clone()).collect();
            Box::new(crate::asking::nobody_called(name, &there))
        })
}

/// What a choice comes to, as the line an operator reads it back in.
fn now_reads(wanted: &Asking) -> String {
    let policy = Policy::of(wanted);
    match wanted.quota {
        Some(quota) if policy.arrives_unseen() => format!(
            "may ask for {} without anybody seeing it first",
            crate::asking::limit(quota)
        ),
        Some(quota) => format!(
            "waits for you, and is counted against {}",
            crate::asking::limit(quota)
        ),
        None => format!("now {}", policy.means()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lemonfiber_fixtures::http::{Answer, Fake};

    use super::{allowing, deciding, now_reads, settled, theirs, Ctx};
    use crate::app::command::{Answer as Ruling, Chosen, Decision};
    use crate::asking::Policy;
    use crate::ports::http::Method;
    use crate::ports::service::{Asking, Quota};
    use crate::test_support::{a_context, a_password, SeedFs};

    /// A household trusted within five a week.
    const FIVE_A_WEEK: Quota = Quota {
        requests: 5,
        days: 7,
    };

    /// A Servarr config that opens a target, carrying a readable key.
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

    /// The two accounts the media server holds — one ordinary, one nobody by that
    /// name, so the forgiving match has something to be forgiving about.
    const ACCOUNTS: &str = r#"[{"Id":"a1","Name":"Alex","HasPassword":true,
        "Policy":{"EnableAllFolders":true},"LastActivityDate":"2026-08-30T10:00:00Z"}]"#;

    /// One request nobody has ruled on, as the request service records it.
    const WAITING: &str = r#"{"pageInfo":{"results":1},"results":[{"id":7,
        "createdAt":"2026-08-17T21:04:09.000Z","status":1,"type":"movie",
        "media":{"status":2,"externalServiceId":3},
        "requestedBy":{"displayName":"Alex"}}]}"#;

    /// A context over a transport that answers every read and write these take.
    ///
    /// Routed by what each call asks for rather than scripted in turn, because the
    /// answer here is the household read *again* — so the same paths are asked twice
    /// and a queue would run out halfway through the second reading.
    fn a_household(tag: &str) -> Ctx {
        answering(tag, Vec::new())
    }

    /// The same household, with the named calls answering a refusal instead.
    ///
    /// One rule per call rather than one transport per case: what each of these holds
    /// is that a service which will not answer *that* leaves the household as it was,
    /// and a fixture that broke everything at once could not say which call it was.
    fn refusing(tag: &str, broken: Vec<(Method, &'static str)>) -> Ctx {
        answering(
            tag,
            broken
                .into_iter()
                .map(|(method, route)| (Some(method), route, Answer::reply(500, "no")))
                .collect(),
        )
    }

    /// The transport these run against, with the broken rules ahead of the working
    /// ones — the first rule whose fragment the address holds is the one that answers.
    fn answering(tag: &str, broken: Vec<(Option<Method>, &'static str, Answer)>) -> Ctx {
        let mut routes = broken;
        routes.extend(vec![
            // Ahead of `/Users`, whose text it contains: a route matched by prefix
            // would answer the sign-in with the list of accounts.
            (
                None,
                "/Users/AuthenticateByName",
                Answer::reply(200, r#"{"AccessToken":"token"}"#),
            ),
            (
                None,
                "/Library/MediaFolders",
                Answer::reply(200, r#"{"Items":[]}"#),
            ),
            (
                None,
                "/Localization/ParentalRatings",
                Answer::reply(200, "[]"),
            ),
            (None, "/Users", Answer::reply(200, ACCOUNTS)),
            (None, "/auth/jellyfin", Answer::reply(200, "{}")),
            (
                None,
                "/settings/main",
                Answer::reply(
                    200,
                    r#"{"defaultPermissions":160,
                        "defaultQuotas":{"movie":{},"tv":{}}}"#,
                ),
            ),
            (
                None,
                "/user/jellyfin/",
                Answer::reply(200, r#"{"id":4,"permissions":160}"#),
            ),
            (
                None,
                "/user/4/quota",
                Answer::reply(
                    200,
                    r#"{"movie":{"days":7,"limit":5,"used":1},"tv":{"days":7,"limit":0,"used":0}}"#,
                ),
            ),
            (
                None,
                "settings/permissions",
                Answer::reply(200, r#"{"permissions":160}"#),
            ),
            (None, "/request/7/", Answer::reply(200, "{}")),
            (None, "/api/v1/request", Answer::reply(200, WAITING)),
            (None, "", Answer::reply(200, "[]")),
        ]);
        let transport = Fake::by_rules(routes);
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-asking-{tag}-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut context = a_context()
            .build()
            .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
            .with_http(transport);
        context.settings.env_file = Some(dir.join(".env"));
        crate::app::targets::record_secret(
            &context,
            crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
            &a_password(),
        );
        context
    }

    /// A choice for the whole household is written and read back as the household.
    #[tokio::test]
    async fn a_choice_for_the_household_answers_with_the_household() {
        let report = allowing(
            &a_household("house"),
            &Chosen {
                member: None,
                policy: Some(Policy::WithinALimit),
                quota: Some(FIVE_A_WEEK),
            },
        )
        .await
        .unwrap_or_default();

        let said = report.findings.first().cloned().unwrap_or_default();
        assert!(said.contains("5 requests a week"), "{said}");
        assert!(said.starts_with("the household"), "{said}");
    }

    /// A choice for one person names them, and leaves the household's own alone.
    #[tokio::test]
    async fn a_choice_for_one_person_names_them() {
        let report = allowing(
            &a_household("person"),
            &Chosen {
                member: Some("alex".to_owned()),
                policy: Some(Policy::EverythingWaits),
                quota: None,
            },
        )
        .await
        .unwrap_or_default();

        let said = report.findings.first().cloned().unwrap_or_default();
        assert!(said.starts_with("Alex"), "{said}");
        assert!(said.contains("waits for you"), "{said}");
        // Their own limit and not the household's. This fixture's household has none
        // and this member has five a week, so the figure in the line is the whole of
        // what says which of the two was read before anything was written.
        assert!(said.contains("5 requests a week"), "{said}");
    }

    /// A name with no media server behind it is refused, not searched.
    ///
    /// The name is matched against the accounts the server holds, so where there is
    /// no server there is nothing to match against. Said as the refusal the rest of
    /// this errand gives rather than as an empty answer, which would read as nobody
    /// being called that.
    #[tokio::test]
    async fn a_name_with_no_media_server_behind_it_is_refused() {
        let refused = found(&a_household("noserver"), &[], "alex").await;

        assert_eq!(
            refused.err().map(|problem| problem.code),
            Some(crate::asking::UNREACHABLE)
        );
    }

    /// Nobody by that name is refused with the household named beside it.
    #[tokio::test]
    async fn nobody_by_that_name_is_refused() {
        let refused = allowing(
            &a_household("nobody"),
            &Chosen {
                member: Some("sam".to_owned()),
                policy: Some(Policy::Trusted),
                quota: None,
            },
        )
        .await;

        assert_eq!(
            refused.err().map(|problem| problem.code),
            Some(crate::asking::NOBODY)
        );
    }

    /// A rehearsal says what it would do and writes nothing.
    #[tokio::test]
    async fn a_rehearsal_says_what_it_would_do_and_writes_nothing() {
        let rehearsing = a_household("rehearsed").rehearsing();

        let report = allowing(
            &rehearsing,
            &Chosen {
                member: None,
                policy: Some(Policy::Trusted),
                quota: None,
            },
        )
        .await
        .unwrap_or_default();
        let one_person = allowing(
            &rehearsing,
            &Chosen {
                member: Some("alex".to_owned()),
                policy: Some(Policy::Trusted),
                quota: None,
            },
        )
        .await
        .unwrap_or_default();

        for said in [report, one_person] {
            let first = said.findings.first().cloned().unwrap_or_default();
            assert!(first.contains("rehearsed"), "{first}");
        }
    }

    /// Approving a waiting request answers with the household and says what happened.
    #[tokio::test]
    async fn approving_a_waiting_request_says_what_happened() {
        let report = deciding(
            &a_household("approve"),
            &Decision {
                request: 7,
                answer: Ruling::LetThrough,
            },
        )
        .await
        .unwrap_or_default();

        let said = report.findings.first().cloned().unwrap_or_default();
        assert!(said.contains("Alex"), "{said}");
        assert!(said.contains("approved"), "{said}");
    }

    /// Turning one down says the reason back, and says who has to carry it.
    #[tokio::test]
    async fn turning_one_down_says_the_reason_back() {
        let report = deciding(
            &a_household("decline"),
            &Decision {
                request: 7,
                answer: Ruling::TurnedDown {
                    reason: "no room this month".to_owned(),
                },
            },
        )
        .await
        .unwrap_or_default();

        let said = report.findings.first().cloned().unwrap_or_default();
        assert!(said.contains("no room this month"), "{said}");
        assert!(said.contains("yours to pass on"), "{said}");
    }

    /// A rehearsed decision says what it would do and rules on nothing.
    #[tokio::test]
    async fn a_rehearsed_decision_rules_on_nothing() {
        let report = deciding(
            &a_household("norule").rehearsing(),
            &Decision {
                request: 7,
                answer: Ruling::LetThrough,
            },
        )
        .await
        .unwrap_or_default();

        let said = report.findings.first().cloned().unwrap_or_default();
        assert!(said.contains("nothing was decided"), "{said}");
    }

    /// A call that will not answer leaves the household as it was, and says so.
    ///
    /// Every write here reaches the service more than once — read what is held, write
    /// what was chosen — and each of those is a separate way for it to go quiet. What
    /// they share is the answer: nothing was changed, which is the one thing an
    /// operator has to be able to believe.
    #[tokio::test]
    async fn a_call_that_will_not_answer_leaves_the_household_as_it_was() {
        let chosen = Chosen {
            member: None,
            policy: Some(Policy::Trusted),
            quota: None,
        };
        let broken = [
            ("read", vec![(Method::Get, "/settings/main")]),
            ("write", vec![(Method::Post, "/settings/main")]),
        ];

        for (tag, rules) in broken {
            let refused = allowing(&refusing(tag, rules), &chosen).await;

            assert_eq!(
                refused.err().map(|problem| problem.code),
                Some(crate::asking::UNREACHABLE),
                "the {tag} half claimed a change it did not make"
            );
        }
    }

    /// The same for a choice about one person, on each of the three calls it makes.
    #[tokio::test]
    async fn a_choice_about_one_person_refuses_on_any_of_its_three_calls() {
        let chosen = Chosen {
            member: Some("alex".to_owned()),
            policy: Some(Policy::WithinALimit),
            quota: Some(FIVE_A_WEEK),
        };
        let broken = [
            ("held", vec![(Method::Get, "/user/jellyfin/")]),
            ("left", vec![(Method::Get, "/user/4/quota")]),
            ("quota", vec![(Method::Post, "/user/4/settings/main")]),
            ("approval", vec![(Method::Post, "settings/permissions")]),
        ];

        for (tag, rules) in broken {
            let refused = allowing(&refusing(tag, rules), &chosen).await;

            assert_eq!(
                refused.err().map(|problem| problem.code),
                Some(crate::asking::UNREACHABLE),
                "the {tag} call claimed a change it did not make"
            );
        }
    }

    /// A stack with no media server has nobody to match a name against.
    ///
    /// Built by taking the shipped stack and removing the one service the household is
    /// read from, so what is under test is the absence rather than a hand-written
    /// manifest that might differ in some other way too.
    #[tokio::test]
    async fn a_stack_with_no_media_server_has_nobody_to_name() {
        static WITHOUT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
        let dir = WITHOUT.get_or_init(|| {
            let from = std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/media-stack"
            ));
            let to = std::env::temp_dir().join(format!(
                "lemonfiber-asking-no-server-{}",
                std::process::id()
            ));
            let _ = std::fs::create_dir_all(&to);
            let read = std::fs::read_to_string(from.join("stack.toml")).unwrap_or_default();
            let kept: String = read
                .split("[[service]]")
                .filter(|block| !block.contains("id = \"jellyfin\""))
                .collect::<Vec<_>>()
                .join("[[service]]");
            let _ = std::fs::write(to.join("stack.toml"), kept);
            to
        });
        let mut ctx = a_household("noserver");
        ctx.stack = crate::stack::Source::External(Box::leak(dir.clone().into_boxed_path()));

        let refused = allowing(
            &ctx,
            &Chosen {
                member: Some("alex".to_owned()),
                policy: Some(Policy::Trusted),
                quota: None,
            },
        )
        .await;

        assert_eq!(
            refused.err().map(|problem| problem.code),
            Some(crate::asking::UNREACHABLE)
        );
    }

    /// Somebody the request service has never heard of is an invitation nobody used.
    ///
    /// Not a fault and not a member who is missing: the service learns of somebody the
    /// first time they sign in, so until then there is nobody there to hold to a limit.
    #[tokio::test]
    async fn somebody_the_request_service_never_heard_of_has_no_limit_to_set() {
        let ctx = answering(
            "unknown",
            vec![(None, "/user/jellyfin/", Answer::reply(404, "no such user"))],
        );

        let refused = allowing(
            &ctx,
            &Chosen {
                member: Some("alex".to_owned()),
                policy: Some(Policy::Trusted),
                quota: None,
            },
        )
        .await;

        assert_eq!(
            refused.err().map(|problem| problem.code),
            Some(crate::asking::NEVER_HERE)
        );
    }

    /// A media server that will not say who is here changes nothing.
    ///
    /// Who is in the household is that server's fact, and a choice about one person
    /// cannot be written against a name nobody confirmed is here.
    #[tokio::test]
    async fn a_media_server_that_will_not_say_who_is_here_changes_nothing() {
        let ctx = refusing(
            "nohousehold",
            vec![(Method::Post, "/Users/AuthenticateByName")],
        );

        let refused = allowing(
            &ctx,
            &Chosen {
                member: Some("alex".to_owned()),
                policy: Some(Policy::Trusted),
                quota: None,
            },
        )
        .await;

        assert_eq!(
            refused.err().map(|problem| problem.code),
            Some(crate::asking::UNREACHABLE)
        );
    }

    /// A request service that will not list what was asked for decides nothing.
    #[tokio::test]
    async fn a_service_that_will_not_list_what_was_asked_for_decides_nothing() {
        let refused = deciding(
            &refusing("nolist", vec![(Method::Get, "/api/v1/request")]),
            &Decision {
                request: 7,
                answer: Ruling::LetThrough,
            },
        )
        .await;

        assert_eq!(
            refused.err().map(|problem| problem.code),
            Some(crate::asking::UNREACHABLE)
        );
    }

    /// A service that will not rule on it says so rather than reporting it decided.
    #[tokio::test]
    async fn a_service_that_will_not_rule_says_so() {
        let refused = deciding(
            &refusing("norule2", vec![(Method::Post, "/request/7/")]),
            &Decision {
                request: 7,
                answer: Ruling::LetThrough,
            },
        )
        .await;

        assert_eq!(
            refused.err().map(|problem| problem.code),
            Some(crate::asking::UNREACHABLE)
        );
    }

    /// A request nobody is waiting on is named rather than ruled on twice.
    #[tokio::test]
    async fn a_request_nobody_is_waiting_on_is_named() {
        let refused = deciding(
            &a_household("missing"),
            &Decision {
                request: 99,
                answer: Ruling::LetThrough,
            },
        )
        .await;

        assert_eq!(
            refused.err().map(|problem| problem.code),
            Some(crate::asking::NOT_WAITING)
        );
    }

    /// What the service holds before a choice is made.
    const fn holding(approves_own: bool, quota: Option<Quota>) -> Asking {
        Asking {
            approves_own,
            quota,
        }
    }

    /// Naming only a limit leaves the policy where it was.
    ///
    /// A run that said nothing about the policy is not a run that chose one, and a
    /// value written for it would be this code deciding on the household's behalf.
    #[test]
    fn naming_only_a_limit_leaves_the_policy_where_it_was() {
        let held = holding(false, None);

        let wanted = settled(
            &Chosen {
                quota: Some(FIVE_A_WEEK),
                ..Chosen::default()
            },
            &held,
        )
        .unwrap_or(held);

        assert!(!wanted.approves_own, "the policy moved");
        assert_eq!(wanted.quota, Some(FIVE_A_WEEK));
    }

    /// Naming only a policy leaves the limit where it was.
    #[test]
    fn naming_only_a_policy_leaves_the_limit_where_it_was() {
        let held = holding(false, Some(FIVE_A_WEEK));

        let wanted = settled(
            &Chosen {
                policy: Some(Policy::WithinALimit),
                ..Chosen::default()
            },
            &held,
        )
        .unwrap_or(held);

        assert!(wanted.approves_own);
        assert_eq!(wanted.quota, Some(FIVE_A_WEEK));
    }

    /// Trusting everybody lifts the limit rather than leaving one counting unseen.
    ///
    /// A household told nothing limits it while the service goes on counting is two
    /// answers to one question, and the one the operator reads is the wrong one.
    #[test]
    fn trusting_everybody_lifts_the_limit_rather_than_leaving_it_counting() {
        let held = holding(true, Some(FIVE_A_WEEK));

        let wanted = settled(
            &Chosen {
                policy: Some(Policy::Trusted),
                ..Chosen::default()
            },
            &held,
        )
        .unwrap_or(held);

        assert_eq!(wanted.quota, None);
        assert!(wanted.approves_own);
    }

    /// Living within a limit with none named, and none in force, is refused.
    #[test]
    fn living_within_a_limit_with_none_anywhere_is_refused() {
        let refused = settled(
            &Chosen {
                policy: Some(Policy::WithinALimit),
                ..Chosen::default()
            },
            &holding(false, None),
        );

        assert_eq!(
            refused.err().map(|problem| problem.code),
            Some(crate::asking::NO_LIMIT)
        );
    }

    /// The same policy with a limit already in force is not refused.
    ///
    /// Choosing to live within the limit you already have is a request, and refusing
    /// it would make the operator retype a number the service already holds.
    #[test]
    fn the_same_policy_over_a_limit_already_in_force_is_not_refused() {
        let wanted = settled(
            &Chosen {
                policy: Some(Policy::WithinALimit),
                ..Chosen::default()
            },
            &holding(false, Some(FIVE_A_WEEK)),
        );

        assert!(wanted.is_ok(), "{wanted:?}");
    }

    /// What one member is under is read off their own counts, and off whichever of
    /// the two halves carries a limit.
    ///
    /// A household chooses one figure and both halves are written together, so the
    /// one that is set is the one read back — and a member with no limit of their own
    /// reads as having none rather than as having the household's.
    #[test]
    fn what_one_member_is_under_is_read_off_their_own_counts() {
        let counted = |limit, days| crate::ports::service::Left {
            limit,
            used: 0,
            days,
        };

        let films = theirs(
            true,
            crate::ports::service::Headroom {
                films: counted(Some(3), Some(7)),
                television: counted(None, None),
            },
        );
        assert_eq!(
            films.quota,
            Some(Quota {
                requests: 3,
                days: 7
            })
        );
        assert!(films.approves_own);

        let television = theirs(
            false,
            crate::ports::service::Headroom {
                films: counted(None, None),
                television: counted(Some(4), Some(30)),
            },
        );
        assert_eq!(
            television.quota,
            Some(Quota {
                requests: 4,
                days: 30
            })
        );
        assert!(!television.approves_own);

        assert_eq!(
            theirs(true, crate::ports::service::Headroom::default()).quota,
            None
        );
    }

    /// Each arrangement reads back as its own line.
    #[test]
    fn each_arrangement_reads_back_as_its_own_line() {
        assert_eq!(
            now_reads(&holding(true, Some(FIVE_A_WEEK))),
            "may ask for 5 requests a week without anybody seeing it first"
        );
        assert!(now_reads(&holding(false, Some(FIVE_A_WEEK))).starts_with("waits for you"));
        assert!(now_reads(&holding(true, None)).contains("everything anybody asks for"));
        assert!(now_reads(&holding(false, None)).contains("until you have said yes"));
    }
}
