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
mod tests {
    use super::{carrying, Told, WHY};
    use crate::outbound::{PUSHBULLET, PUSHOVER};
    use crate::ports::http::Method;
    use crate::ports::service::Address;

    /// A member reached on Pushover, with a sound of their own.
    fn pushover(sound: Option<&str>) -> Address {
        Address::Pushover {
            user: "the-user-key".to_owned(),
            application: "the-application-token".to_owned(),
            sound: sound.map(str::to_owned),
        }
    }

    /// The same member reached on Pushbullet.
    fn pushbullet() -> Address {
        Address::Pushbullet {
            token: "the-access-token".to_owned(),
        }
    }

    /// A message goes where the enumeration says it can go, and nowhere else.
    ///
    /// Read off the list rather than off a literal here, because that is the whole of
    /// what makes the list a property of the program: an address written into this file
    /// and left off the enumeration would pass a test that named it twice.
    #[test]
    fn a_message_goes_only_where_the_enumeration_names() {
        let allowed: Vec<String> =
            crate::outbound::leaving(&crate::config::Settings::default(), &[])
                .ours
                .into_iter()
                .filter(|entry| entry.reach == crate::outbound::Reach::Household)
                .flat_map(|entry| entry.destination)
                .collect();

        assert_eq!(allowed.len(), 2, "{allowed:?}");
        for address in [pushover(Some("bike")), pushbullet()] {
            let asked = carrying(&address, "no room this month");
            assert!(
                allowed.contains(&asked.url),
                "{} is sent somewhere the operator was not told about: {}",
                address.service(),
                asked.url
            );
            assert_eq!(asked.method, Method::Post);
        }
    }

    /// What travels is the reason under one word, and the member's own token.
    #[test]
    fn what_travels_is_the_reason_and_the_members_own_token() {
        let asked = carrying(&pushover(Some("bike")), "no room this month");
        let sent = asked.body.unwrap_or_default();

        assert_eq!(asked.url, PUSHOVER);
        assert!(sent.contains(r#""message":"no room this month""#), "{sent}");
        assert!(sent.contains(&format!(r#""title":"{WHY}""#)), "{sent}");
        assert!(sent.contains(r#""user":"the-user-key""#), "{sent}");
        assert!(
            sent.contains(r#""token":"the-application-token""#),
            "{sent}"
        );
        assert!(sent.contains(r#""sound":"bike""#), "{sent}");
        // The name of this program, an address to open, and the word the request
        // service has already sent them. None of the three has any business here.
        for absent in ["lemonfiber", "http://", "declined"] {
            assert!(!sent.contains(absent), "{absent} travelled: {sent}");
        }
    }

    /// A member who chose no sound is sent none rather than a blank one.
    #[test]
    fn no_sound_chosen_is_no_sound_sent() {
        let sent = carrying(&pushover(None), "not this month")
            .body
            .unwrap_or_default();

        assert!(!sent.contains("sound"), "{sent}");
    }

    /// Pushbullet takes its token in a header and the reason in a note.
    #[test]
    fn pushbullet_carries_its_token_in_a_header() {
        let asked = carrying(&pushbullet(), "we already have it");
        let sent = asked.body.clone().unwrap_or_default();

        assert_eq!(asked.url, PUSHBULLET);
        assert!(sent.contains(r#""type":"note""#), "{sent}");
        assert!(sent.contains(r#""body":"we already have it""#), "{sent}");
        assert!(
            asked
                .headers
                .iter()
                .any(|(name, value)| name == "Access-Token" && value == "the-access-token"),
            "{:?}",
            asked.headers
        );
        assert!(!sent.contains("the-access-token"), "{sent}");
    }

    /// Nowhere to send is told apart from everywhere refusing.
    #[test]
    fn nowhere_to_send_is_not_everything_refusing() {
        let nowhere = Told::default();
        let refused = Told {
            reached: Vec::new(),
            refused: vec!["Pushover".to_owned()],
        };
        let reached = Told {
            reached: vec!["Pushbullet".to_owned()],
            refused: Vec::new(),
        };

        assert!(nowhere.nowhere());
        assert!(!refused.nowhere());
        assert!(!reached.nowhere());
        assert_eq!(nowhere, nowhere.clone());
        assert_eq!(format!("{nowhere:?}"), format!("{:?}", Told::default()));
    }
}
