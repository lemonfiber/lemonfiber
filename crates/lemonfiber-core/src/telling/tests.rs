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
        crate::outbound::leaving(&crate::config::Settings::default(), &[], &[])
            .ours
            .into_iter()
            .filter(|entry| entry.reach == crate::outbound::Reach::Household)
            .flat_map(|entry| entry.destination)
            .collect();

    assert_eq!(allowed.len(), 2, "{allowed:?}");
    for address in [pushover(Some("bike")), pushbullet()] {
        let asked = carrying(&address, "no room this month");
        // Bound rather than named inside the assertion's own message, which is
        // evaluated only where the assertion fails: a call that lives there is a
        // call nothing ever makes.
        let service = address.service();
        assert!(
            allowed.contains(&asked.url),
            "{service} is sent somewhere the operator was not told about: {}",
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
    assert!(
        sent.contains(r#""message":"no room this month""#),
        "the reason did not travel"
    );
    assert!(
        sent.contains(&format!(r#""title":"{WHY}""#)),
        "the one word over the reason did not travel"
    );
    assert!(
        sent.contains(r#""user":"the-user-key""#),
        "the member's own key did not travel"
    );
    assert!(
        sent.contains(r#""token":"the-application-token""#),
        "the application token did not travel"
    );
    assert!(
        sent.contains(r#""sound":"bike""#),
        "the sound the member chose did not travel"
    );
    // The name of this program, an address to open, and the word the request
    // service has already sent them. None of the three has any business here.
    for absent in ["lemonfiber", "http://", "declined"] {
        assert!(!sent.contains(absent), "{absent} travelled");
    }
}

/// A member who chose no sound is sent none rather than a blank one.
#[test]
fn no_sound_chosen_is_no_sound_sent() {
    let sent = carrying(&pushover(None), "not this month")
        .body
        .unwrap_or_default();

    assert!(
        !sent.contains("sound"),
        "a member who chose no sound was sent a blank one"
    );
}

/// Pushbullet takes its token in a header and the reason in a note.
#[test]
fn pushbullet_carries_its_token_in_a_header() {
    let asked = carrying(&pushbullet(), "we already have it");
    let sent = asked.body.clone().unwrap_or_default();

    assert_eq!(asked.url, PUSHBULLET);
    assert!(
        sent.contains(r#""type":"note""#),
        "the note did not travel as one"
    );
    assert!(
        sent.contains(r#""body":"we already have it""#),
        "the reason did not travel"
    );
    assert!(
        asked
            .headers
            .iter()
            .any(|(name, value)| name == "Access-Token" && value == "the-access-token"),
        "the token did not travel in the header that carries it"
    );
    assert!(
        !sent.contains("the-access-token"),
        "the access token travelled in the body as well as the header"
    );
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
