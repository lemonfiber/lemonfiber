//! Who this surface lets in, and what it keeps in order to decide.
//!
//! Everything here is about the operator's own password — the one credential in
//! this product lemonfiber *verifies* rather than reads. Every other secret the
//! stack holds is a service's own, fetched so lemonfiber can present it; this one
//! arrives from a person and is never given back to anybody, which is a different
//! problem and is why it is kept apart from them.
//!
//! It exists because the web surface can start, stop and reconfigure the whole
//! stack, and because a household reaches lemonfiber from a phone rather than from
//! the machine it runs on. A secret printed on the terminal that started the
//! process answers "whoever is at this machine", which is the population loopback
//! already answers for. A password answers a different question, and it is the one
//! that has to be answered before anything beyond loopback is offered.

pub mod credential;

pub use credential::Credential;
