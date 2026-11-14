use std::sync::Arc;

use lemonfiber_ports::http::Sessions;

use super::Fresh;

#[test]
fn each_session_opens_a_client_of_its_own() {
    assert!(!Arc::ptr_eq(&Fresh.opened(), &Fresh.opened()));
}
