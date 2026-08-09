//! Whether the pipeline is actually moving.
//!
//! Setup is a one-day problem; a queue that quietly jams is the forever one. A
//! download stalls at 94%. An import fails because a permission is wrong, or the
//! release is in a format nothing extracted. An indexer returns nothing because
//! its key expired months ago and it says so by answering politely with an empty
//! list.
//!
//! Each service knows about its own stall and none of them tell anyone. Sonarr
//! will show it if you open the queue and look. The operator's experience is that
//! things stopped appearing, and their diagnosis is "it broke".
//!
//! The failure that matters most belongs to nobody: an item that **downloaded
//! successfully and was never imported**. The client considers it finished. The
//! \*arr never picked it up. From each service's own perspective there is nothing
//! wrong, and only something holding both sides at once can see it. That is why
//! this assesses across services rather than within one.
//!
//! Nothing here reaches a service. Asking them what they have is the app layer's;
//! deciding what their answers add up to is this one's.
//!
//! - [`Item`] — one thing in the pipeline, with what each side said about it.
//! - [`Stall`] — the ways it stops moving, each with its own remedy.
//! - [`Thresholds`] — how long something has to be wrong before anyone is told.
//! - [`assess`] — what, if anything, to say.

mod assess;
mod item;
mod stall;
mod threshold;

pub use assess::{assess, Stuck, LOOPING, REPEATED};
pub use item::{Fetching, Importing, Item};
pub use stall::Stall;
pub use threshold::Thresholds;
