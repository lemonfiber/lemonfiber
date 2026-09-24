use super::{given, would_send, Reachable, DECLINED, PUSHBULLET, PUSHOVER};
use crate::ports::service::Address;

/// A switch carrying only the service's test notification, which is not a refusal.
const ONLY_A_TEST: u64 = 32;

/// Settings as the service answers them, with both agents filled in.
fn held(switch: u64) -> Reachable {
    serde_json::from_str(&format!(
        r#"{{"pushoverUserKey":"the-user-key",
             "pushoverApplicationToken":"the-application-token",
             "pushoverSound":"pushover",
             "pushbulletAccessToken":"the-access-token",
             "notificationTypes":{{"pushover":{switch},"pushbullet":{switch}}}}}"#
    ))
    .unwrap_or_default()
}

/// A member who switched refusals on where they gave an address is reachable there.
#[test]
fn both_addresses_come_back_where_the_member_asked_to_hear_about_a_refusal() {
    let reached = held(DECLINED).addresses();

    assert_eq!(
        reached,
        vec![
            Address::Pushover {
                user: "the-user-key".to_owned(),
                application: "the-application-token".to_owned(),
                sound: Some("pushover".to_owned()),
            },
            Address::Pushbullet {
                token: "the-access-token".to_owned(),
            },
        ]
    );
}

/// An address with the refusal switched off is not somewhere to send.
///
/// Nought is what the service stores for anybody who has never chosen, and it is
/// what it treats as telling them nothing on that agent — so a member with a token
/// and no choice hears from the service there and would hear from here too, which
/// is a message they never asked for.
#[test]
fn an_agent_the_member_left_switched_off_is_not_somewhere_to_send() {
    assert!(held(0).addresses().is_empty(), "nought reached somebody");
    assert!(
        held(ONLY_A_TEST).addresses().is_empty(),
        "a switch carrying only the test notification reached somebody"
    );
    assert!(
        !held(DECLINED + ONLY_A_TEST).addresses().is_empty(),
        "a switch carrying a refusal beside another event reached nobody"
    );
}

/// A member with the switch on and no address is nowhere to send, not a failure.
#[test]
fn a_member_with_no_address_is_nowhere_to_send() {
    let empty: Reachable = serde_json::from_str(&format!(
        r#"{{"pushoverUserKey":"  ","pushbulletAccessToken":null,
             "notificationTypes":{{"{PUSHOVER}":{DECLINED},"{PUSHBULLET}":{DECLINED}}}}}"#
    ))
    .unwrap_or_default();

    assert!(empty.addresses().is_empty());
}

/// Half a Pushover address is not an address.
///
/// The service's own agent sends only where both halves are the member's own, and
/// one half here would be a message addressed to nobody.
#[test]
fn half_a_pushover_address_is_not_one() {
    let half: Reachable = serde_json::from_str(&format!(
        r#"{{"pushoverUserKey":"the-user-key",
             "notificationTypes":{{"{PUSHOVER}":{DECLINED}}}}}"#
    ))
    .unwrap_or_default();

    assert!(half.addresses().is_empty());
}

/// A switch this cannot read is read as nothing switched on.
#[test]
fn a_switch_that_is_not_a_number_reaches_nobody() {
    let odd: Reachable = serde_json::from_str(
        r#"{"pushbulletAccessToken":"the-access-token",
            "notificationTypes":{"pushbullet":"all of them"}}"#,
    )
    .unwrap_or_default();

    assert!(odd.addresses().is_empty());
    assert!(!would_send(0));
}

/// A settings document nothing can read is no addresses rather than a wrong one.
#[test]
fn a_document_that_will_not_read_is_no_addresses() {
    let unread: Reachable = serde_json::from_str("not a document").unwrap_or_default();

    assert!(unread.addresses().is_empty());
    assert_eq!(given(None), None);
}
