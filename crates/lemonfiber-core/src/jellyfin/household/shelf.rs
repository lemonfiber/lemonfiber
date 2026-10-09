//! What one member may watch, asked of the media server about that account.
//!
//! Its own file because it is a different question from the one beside it. That one is
//! who holds an account and what each may do; this is what is *on the shelf* for one of
//! them — and the answer is the server's rather than this product's, which is worth a
//! module of its own to say once.

use super::{item_type, Jellyfin, Kind, Method};
use crate::jellyfin::item::{ItemResource, ItemsResource};
use crate::ports::service::{Failure, Item};

/// What this member may watch, as the server answers it for them.
///
/// Beside the impl rather than inside it because it decides, and nothing inside an
/// `#[async_trait]` body is attributed to a line in the coverage report.
///
/// **Addressed to the member's account.** `/Users/{id}/Items` is the server applying
/// that account's own library access, age limit and blocked kinds before it answers.
/// The administrator's key authenticates the call, but the id in the path is what the
/// limits are read from — so nothing here re-applies them, and there is no second copy
/// of three rules to disagree with the server on the day any of them moves.
///
/// **Naming nobody asks `/Items` instead**, which the server answers for whoever
/// signed in: the administrator, who holds every library and no age limit. That is the
/// access an invitation that chose nothing grants, and asking it this way reads no
/// member's account at all.
pub(super) async fn holdings(
    jellyfin: &Jellyfin,
    member: Option<&str>,
    most: u32,
) -> Result<Vec<Item>, Failure> {
    let response = jellyfin
        .as_admin(Method::Get, &asked(member, most), None)
        .await?;
    let held: ItemsResource = jellyfin
        .endpoint
        .decode(&response, "what the household holds could not be read")?;

    Ok(held.items.into_iter().map(ItemResource::item).collect())
}

/// The path a shelf is asked at, for one member or for nobody in particular.
fn asked(member: Option<&str>, most: u32) -> String {
    let kinds = Kind::ALL.map(item_type).join(",");
    let whose = member.map_or_else(String::new, |member| format!("/Users/{member}"));
    format!(
        "{whose}/Items?Recursive=true&IncludeItemTypes={kinds}\
         &SortBy=DateCreated&SortOrder=Descending&Limit={most}"
    )
}
