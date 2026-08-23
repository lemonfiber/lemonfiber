//! The JSON endpoints lemonfiber answers on loopback.
//!
//! Every endpoint answers with the envelope the equivalent command emits under
//! `--json`, byte for byte. A script piping one and a browser fetching the other
//! receive the same document, so this surface adds no vocabulary of its own and
//! cannot drift from the command line by having its own opinion.
//!
//! What it does add is refusal. A writable API on loopback is reachable from any
//! page the operator visits, which the command line never was, so what a request
//! must carry to be answered is stated here rather than assumed.

pub mod guard;
pub mod read;
pub mod router;
pub mod serve;
