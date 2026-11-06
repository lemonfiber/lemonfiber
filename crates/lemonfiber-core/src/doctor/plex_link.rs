//! How the request service is linked to Plex, reported on every diagnosis: the mode, whose
//! Plex account the request service's owner is, and whether members can sign in to its
//! page with Plex, with `owner`'s cost as a standing notice.

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::error::codes::setting::OWNER_HELD;
use crate::error::{Problem, Remedy};
use crate::plex_link::{Mode, OWNER_COST, SETTING};

/// The name this check's finding carries.
pub(crate) const CHECK: &str = "services.plex-link";

/// The heading the finding is read under.
const TITLE: &str = "How the request service is linked to Plex";

/// The link as the check finds it, where Plex is the media server the gate routes to.
pub(crate) struct Linked {
    /// The id of the request service.
    pub(crate) requests: String,
    /// What `PLEX_LINK_MODE` holds, where anything.
    pub(crate) recorded: Option<String>,
    /// The name of the request service's own Plex account, where it could be read.
    pub(crate) own: Option<String>,
    /// The plex.tv id of the account that claimed Plex, where it could be read.
    pub(crate) claimed_by: Option<String>,
}

/// Reports how the request service is linked to Plex.
pub(crate) struct PlexLinkCheck {
    /// The link, or nothing where Plex is not the media server the gate routes to.
    linked: Option<Linked>,
}

impl PlexLinkCheck {
    /// A check over `linked`.
    #[must_use]
    pub(crate) const fn new(linked: Option<Linked>) -> Self {
        Self { linked }
    }
}

#[async_trait]
impl Check for PlexLinkCheck {
    fn category(&self) -> Category {
        Category::Services
    }

    async fn run(&self) -> Vec<Finding> {
        findings(self.linked.as_ref())
    }
}

/// The one finding `linked` comes to, skipped where there is no link to report.
fn findings(linked: Option<&Linked>) -> Vec<Finding> {
    let Some(linked) = linked else {
        return vec![Finding::in_category(
            Category::Services,
            CHECK,
            TITLE,
            Verdict::Skipped {
                reason: "Plex is not the media server the request service reaches through the \
                         request gate"
                    .to_owned(),
            },
        )];
    };
    vec![
        Finding::in_category(Category::Services, CHECK, TITLE, verdict(linked))
            .about(&linked.requests),
    ]
}

/// What the link comes to.
fn verdict(linked: &Linked) -> Verdict {
    let own = linked.own.as_deref().map_or_else(
        || "its own Plex account".to_owned(),
        |name| format!("its own Plex account, {name}"),
    );
    match Mode::held(linked.recorded.as_deref()) {
        Some(Mode::Managed) => Verdict::Pass {
            note: Some(format!(
                "{SETTING} is managed: the request service's owner is {own}, and members \
                 cannot sign in to its page with Plex; they ask through lemonfiber"
            )),
        },
        Some(Mode::Fronted) => Verdict::Pass {
            note: Some(format!(
                "{SETTING} is fronted: the request service's owner is {own}, holding a gate \
                 token, and members can sign in to its page with Plex, its plex.tv calls \
                 answered by the request gate"
            )),
        },
        Some(Mode::Owner) => Verdict::Warn(owner_held(linked.claimed_by.as_deref())),
        None => Verdict::Unverified {
            reason: crate::plex_link::refusal(linked.recorded.as_deref().unwrap_or_default())
                .unwrap_or_default(),
            remedy: Remedy::new(format!("Set {SETTING} to managed, owner or fronted"))
                .with_detail(format!("lemonfiber settings set {SETTING} managed")),
        },
    }
}

/// The standing notice of `owner` mode, naming the account that claimed Plex by its plex.tv
/// id where it could be read.
fn owner_held(claimed_by: Option<&str>) -> Problem {
    let whose = claimed_by.map_or_else(
        || "your own Plex account".to_owned(),
        |id| format!("your own Plex account (plex.tv id {id})"),
    );
    Problem::new(
        OWNER_HELD,
        format!(
            "{SETTING} is owner: the request service's owner is {whose}, and members can sign \
             in to its page with Plex"
        ),
        format!("As you chose: {OWNER_COST}."),
        Remedy::new("Sign the request service's device out at plex.tv to take the token back"),
    )
}

#[cfg(test)]
mod tests;
