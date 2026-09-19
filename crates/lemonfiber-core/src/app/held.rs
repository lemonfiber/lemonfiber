//! What one member can watch, read from the media server as that member.
//!
//! The household read answers "what have we asked for". This answers "what is already
//! here", which is the other half of what somebody opens the app for and the half this
//! product could not answer at all.
//!
//! **Asked for one member, always.** There is no whole-household form of this and the
//! absence is the design: what is on the shelf is different for every account, because
//! the server applies that account's age limit, blocked kinds and library access before
//! it answers. A single answer for everybody would be a fourth copy of three rules, and
//! it would be wrong for whoever it was not read as. An operator naming a member gets
//! that member's shelf — which is also the only honest way to answer "what can my
//! child actually see".

use super::targets::jellyfin_reader;
use super::Ctx;

use crate::error::{Diagnose, Problem};
use crate::model::HeldReport;
use crate::ports::service::{Household as _, Member};

/// Read what one member holds.
///
/// `member` is matched against the id the server files them under first and against
/// their name second. A member's own session carries the id, so it is the first that
/// answers them; an operator types a name, so it is the second that answers an operator.
/// One resolution for both rather than two paths that could come to disagree about who
/// was meant.
pub(super) async fn held(ctx: &Ctx, member: &str, most: u32) -> Result<HeldReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    let Some(server) = jellyfin_reader(ctx, &manifest.services) else {
        return Ok(unread(
            member,
            "there is no media server to ask what the household holds, or no recorded \
             password to sign in with",
        ));
    };

    let Ok(accounts) = server.household().await else {
        return Ok(unread(
            member,
            "the media server would not say who holds an account, so who this shelf \
             belongs to could not be established",
        ));
    };

    let Some(whose) = whose(&accounts, member) else {
        return Ok(unread(
            member,
            "nobody in this household is known by that, so there is no shelf to read — \
             which is not the same as a shelf with nothing on it",
        ));
    };

    let Ok(holdings) = server.holdings(&whose.id, most).await else {
        return Ok(HeldReport {
            member: whose.name.clone(),
            id: whose.id.clone(),
            findings: vec![
                "the media server would not say what this member holds, so the shelf is \
                 reported as unread rather than as empty"
                    .to_owned(),
            ],
            ..HeldReport::default()
        });
    };

    Ok(HeldReport {
        member: whose.name.clone(),
        id: whose.id.clone(),
        holdings,
        available: true,
        findings: Vec::new(),
    })
}

/// The account a name or an id means, or nobody.
fn whose<'a>(accounts: &'a [Member], named: &str) -> Option<&'a Member> {
    accounts.iter().find(|held| held.id == named).or_else(|| {
        accounts
            .iter()
            .find(|held| held.name.eq_ignore_ascii_case(named))
    })
}

/// A shelf that could not be read, said as that rather than as an empty one.
fn unread(member: &str, why: &str) -> HeldReport {
    HeldReport {
        member: member.to_owned(),
        findings: vec![why.to_owned()],
        ..HeldReport::default()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lemonfiber_fixtures::http::{Answer, Fake as Transport};

    use super::{held, unread, whose, Ctx};
    use crate::ports::service::{Access, Medium, Member};
    use crate::test_support::{a_context, a_password, SeedFs};

    /// A Servarr config carrying a readable key, so the stack resolves its targets.
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

    /// The media server's scripted answers to the two questions this read asks it.
    struct Server {
        /// Who it says holds an account.
        accounts: &'static str,
        /// What it answers about one account's shelf, and with what status.
        shelf: (u16, &'static str),
    }

    impl Default for Server {
        fn default() -> Self {
            Self {
                accounts: r#"[{"Id":"a7f3","Name":"Ada","HasPassword":true,
                    "Policy":{"EnableAllFolders":true}}]"#,
                shelf: (
                    200,
                    r#"{"Items":[
                        {"Id":"i1","Name":"Arrival","ProductionYear":2016,"Type":"Movie"},
                        {"Id":"i2","Name":"The Expanse","Type":"Series"}]}"#,
                ),
            }
        }
    }

    impl Server {
        /// The scripted answers as a transport, routed by what each call asks for.
        ///
        /// The shelf sits ahead of the account list because `/Users/{id}/Items`
        /// contains both fragments, and behind the sign-in for the same reason.
        fn transport(&self) -> Arc<Transport> {
            Transport::by_path(vec![
                (
                    "/Users/AuthenticateByName",
                    Answer::reply(200, r#"{"AccessToken":"token"}"#),
                ),
                ("/Items", Answer::reply(self.shelf.0, self.shelf.1)),
                ("/Users", Answer::reply(200, self.accounts)),
                ("", Answer::reply(200, "[]")),
            ])
        }
    }

    /// A context whose media server can be reached: the admin password is recorded, so
    /// `jellyfin_reader` resolves a client. Tagged so each case keeps its own env file
    /// rather than racing on a shared one.
    fn ctx_with(server: &Server, tag: &str) -> Ctx {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-held-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let mut context = a_context()
            .build()
            .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
            .with_http(server.transport());
        context.settings.env_file = Some(dir.join(".env"));
        crate::app::targets::record_secret(
            &context,
            crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
            &a_password(),
        );
        context
    }

    fn account(id: &str, name: &str) -> Member {
        Member {
            id: id.to_owned(),
            name: name.to_owned(),
            claimed: true,
            access: Access::default(),
            last_seen: None,
        }
    }

    fn household() -> Vec<Member> {
        vec![account("a7f3", "Ada"), account("b2e9", "Bram")]
    }

    /// A member's own session carries the id, so the id is what has to answer — and it
    /// has to answer before the name does, or a household where somebody is called by
    /// another member's id would hand over the wrong shelf.
    #[test]
    fn an_id_names_whose_shelf_it_is() {
        assert_eq!(
            whose(&household(), "a7f3").map(|held| &held.name),
            Some(&"Ada".to_owned())
        );
    }

    /// An operator types a name, and types it the way they say it.
    #[test]
    fn a_name_names_it_too_however_it_was_typed() {
        assert_eq!(
            whose(&household(), "BRAM").map(|held| &held.id),
            Some(&"b2e9".to_owned())
        );
    }

    #[test]
    fn nobody_of_that_name_is_nobody() {
        assert!(whose(&household(), "Cleo").is_none());
    }

    /// The load-bearing one. Everything that could not be read answers through here, and
    /// what it must never do is come back looking like a household that owns nothing.
    #[test]
    fn a_shelf_that_could_not_be_read_says_so_rather_than_reading_as_empty() {
        let report = unread("Ada", "the server would not say");
        assert!(!report.available);
        assert!(report.holdings.is_empty());
        assert_eq!(report.findings, vec!["the server would not say".to_owned()]);
    }

    /// The whole read, as the server answers it.
    ///
    /// Asserted on the items rather than only on the mark, because a report marked
    /// readable and carrying nothing is the exact reading this must never produce.
    #[tokio::test]
    async fn a_shelf_that_reads_comes_back_as_the_server_answered_it() {
        let context = ctx_with(&Server::default(), "read");

        let report = held(&context, "Ada", 100).await.unwrap_or_default();

        assert!(report.available, "{report:?}");
        assert_eq!(report.member, "Ada");
        assert_eq!(report.id, "a7f3");
        assert!(report.findings.is_empty(), "{report:?}");
        assert_eq!(
            report
                .holdings
                .iter()
                .map(|held| (held.title.clone(), held.year, held.medium))
                .collect::<Vec<_>>(),
            vec![
                ("Arrival".to_owned(), Some(2016), Medium::Film),
                ("The Expanse".to_owned(), None, Medium::Series),
            ]
        );
    }

    /// Asked for by the id a member's own session carries, rather than by the name an
    /// operator types. One shelf either way, because one resolution answers both.
    #[tokio::test]
    async fn the_same_shelf_answers_an_id_as_answers_a_name() {
        let context = ctx_with(&Server::default(), "by-id");

        let report = held(&context, "a7f3", 100).await.unwrap_or_default();

        assert!(report.available, "{report:?}");
        assert_eq!(report.member, "Ada");
        assert_eq!(report.holdings.len(), 2, "{report:?}");
    }

    /// A media server that will not say who holds an account leaves the shelf unread.
    ///
    /// Not as a household that owns nothing: whose shelf this is was never established,
    /// so there is no account to report an empty one about.
    #[tokio::test]
    async fn a_household_the_server_will_not_name_leaves_the_shelf_unread() {
        let context = ctx_with(
            &Server {
                accounts: "not json",
                ..Server::default()
            },
            "nameless",
        );

        let report = held(&context, "Ada", 100).await.unwrap_or_default();

        assert!(!report.available, "{report:?}");
        assert!(report.holdings.is_empty(), "{report:?}");
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.contains("who holds an account")),
            "{report:?}"
        );
    }

    /// Nobody of that name is said as nobody of that name.
    ///
    /// The distinction the finding has to carry: there being no such member and there
    /// being a member with nothing on their shelf are different facts, and answering
    /// the first with the second tells a household something untrue about somebody who
    /// does not exist.
    #[tokio::test]
    async fn nobody_of_that_name_has_no_shelf_rather_than_an_empty_one() {
        let context = ctx_with(&Server::default(), "stranger");

        let report = held(&context, "Cleo", 100).await.unwrap_or_default();

        assert!(!report.available, "{report:?}");
        assert_eq!(report.member, "Cleo");
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.contains("nobody in this household")),
            "{report:?}"
        );
    }

    /// A shelf the server will not answer still says whose it was.
    ///
    /// Who was asked about was established before the shelf was asked for, so it is
    /// known — and dropping it would make an unread shelf indistinguishable from a
    /// member nobody could find.
    #[tokio::test]
    async fn a_shelf_the_server_will_not_answer_is_unread_and_still_says_whose() {
        let context = ctx_with(
            &Server {
                shelf: (500, ""),
                ..Server::default()
            },
            "refused",
        );

        let report = held(&context, "Ada", 100).await.unwrap_or_default();

        assert!(!report.available, "{report:?}");
        assert!(report.holdings.is_empty(), "{report:?}");
        assert_eq!(report.member, "Ada");
        assert_eq!(report.id, "a7f3");
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.contains("reported as unread rather than as empty")),
            "{report:?}"
        );
    }

    /// A stack that cannot be read is an error rather than a shelf.
    ///
    /// The one refusal here that is not a finding: without a manifest there is no media
    /// server to address the question to, so there is nothing to report about.
    #[tokio::test]
    async fn a_shelf_over_an_unreadable_stack_is_an_error() {
        let mut context = ctx_with(&Server::default(), "badstack");
        context.stack = crate::stack::Source::External(std::path::Path::new("/nowhere/at/all"));

        assert!(held(&context, "Ada", 100).await.is_err());
    }
}
