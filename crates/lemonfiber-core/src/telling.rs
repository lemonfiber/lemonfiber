//! Carrying one sentence to a household member, at an address they already gave.
//!
//! **This is the only thing lemonfiber sends to a person.** Everything else it asks of
//! the world proves a credential, fetches an image or reads a public address; this
//! carries somebody's words to somebody else, which is why it is the sixth entry in
//! [`crate::outbound`] and why it has a switch of its own.
//!
//! **What travels is the reason and nothing else.** The request service already tells
//! whoever asked that their request was declined, and a second message saying so would
//! be the duplicate this product refuses to send. What that service has nowhere to put
//! is *why*, so why is the whole of what goes — under the word `Why`, with no name of
//! this program, no address to open and nothing to sign in to, because the household
//! has no account here and learning of one from a notification is not how they would
//! want to.
//!
//! **Where it goes is not decided here.** The two addresses are declared in the
//! enumeration an operator reads and handed to the request built below, so a message
//! cannot reach anywhere the list does not name. The member's own address and token come
//! from the request service, read through [`crate::ports::service::Addressing`].
//!
//! **Nothing is retried and a refusal is not a failure.** The decision it accompanies has
//! already been taken at the request service, and a message that would not go is a thing
//! to tell the operator about rather than a reason to undo somebody's answer — so what
//! this returns is what happened, and the caller says it.

use std::sync::Arc;

use crate::endpoint::json_content_type;
use crate::outbound::{PUSHBULLET, PUSHOVER};
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::Address;

/// The word the reason arrives under.
///
/// One word, and deliberately not a sentence: whoever reads it has just been told their
/// request was declined by the service itself, and what is missing from that is the
/// reason rather than another way of saying the same thing.
const WHY: &str = "Why";

/// The header Pushbullet takes its token in.
const PUSHBULLET_TOKEN: &str = "Access-Token";

/// What became of telling one member.
///
/// Both halves are named services rather than counts, because what an operator does
/// about this differs by which: nothing reached is a reason to pass the words on
/// themselves, and one service refusing while another took it is not.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Told {
    /// The services it reached, by the names the member knows them by.
    pub reached: Vec<String>,
    /// The services that would not take it.
    pub refused: Vec<String>,
}

impl Told {
    /// Whether there was nowhere at all to send.
    ///
    /// Different from everything refusing: a member with no address of these two kinds
    /// is the ordinary case and not a fault, and one whose service would not answer is
    /// something an operator may want to look at.
    #[must_use]
    pub fn nowhere(&self) -> bool {
        self.reached.is_empty() && self.refused.is_empty()
    }
}

/// Carry one reason to every address this member gave.
///
/// Each address is its own send. One service refusing does not stop the next, because
/// the point of a member having named two is that either of them reaches them.
pub async fn tell(http: &Arc<dyn Http>, addresses: &[Address], reason: &str) -> Told {
    let mut told = Told::default();
    for address in addresses {
        let took = http
            .send(&carrying(address, reason))
            .await
            .is_ok_and(|answered| answered.is_success());
        let service = address.service().to_owned();
        if took {
            told.reached.push(service);
        } else {
            told.refused.push(service);
        }
    }
    told
}

/// The one request that carries a reason to one address.
///
/// Shaped as the request service's own agents shape theirs, so what arrives arrives in
/// the place and the form that member already receives things in.
fn carrying(address: &Address, reason: &str) -> Request {
    let (url, body, token) = match address {
        Address::Pushover {
            user,
            application,
            sound,
        } => (
            PUSHOVER,
            pushover_body(user, application, sound.as_deref(), reason),
            None,
        ),
        Address::Pushbullet { token } => (
            PUSHBULLET,
            serde_json::json!({ "type": "note", "title": WHY, "body": reason }).to_string(),
            Some(token.clone()),
        ),
    };
    let mut headers: Vec<(String, String)> = json_content_type(Some(&body)).into_iter().collect();
    headers.extend(token.map(|token| (PUSHBULLET_TOKEN.to_owned(), token)));
    Request {
        method: Method::Post,
        url: url.to_owned(),
        headers,
        body: Some(body),
    }
}

/// Pushover's own body, with the sound left out where the member chose none.
///
/// Left out rather than sent empty, which is what the request service's own agent does
/// with the same field: it passes whatever the member chose and passes nothing where
/// they chose nothing, so a message from here sounds like the ones they already get.
fn pushover_body(user: &str, application: &str, sound: Option<&str>, reason: &str) -> String {
    let mut body = serde_json::Map::new();
    body.insert("token".to_owned(), application.into());
    body.insert("user".to_owned(), user.into());
    body.insert("title".to_owned(), WHY.into());
    body.insert("message".to_owned(), reason.into());
    if let Some(sound) = sound {
        body.insert("sound".to_owned(), sound.into());
    }
    serde_json::Value::Object(body).to_string()
}

#[cfg(test)]
mod tests;
