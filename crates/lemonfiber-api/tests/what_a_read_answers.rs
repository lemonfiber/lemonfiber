//! What each read answers with, asked for through the router a request meets.
//!
//! Driven through the assembled router rather than by calling a handler, because
//! what a caller can reach is the thing worth holding still — and because the
//! guard every endpoint sits behind is part of what an endpoint answers.

mod reading;

use reading::*;

mod choices;
mod items;
mod reaches;
mod stack;
