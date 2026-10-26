//! What the request service will let the household ask for, driven through the
//! dispatcher against the request service's adapter, faked as a transport answering
//! `request.intake`.
//!
//! **Every fixture answers by route rather than in turn.** Two of these calls read
//! before they write, so a queue would prove only that the right number of requests went
//! out.
//!
//! The scripted service and the installs that reach it are shared with the file next
//! door, which drives the other half of the same exchange: what becomes of the reason
//! a refusal carried.

use crate::common;

mod deciding;
mod limits;
