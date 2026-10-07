//! Nobody to ask.

use super::{Asking, Nobody};

#[test]
fn nobody_is_there_and_answers_nothing() {
    assert!(!Nobody.present());
    assert_eq!(Nobody.ask("anything?"), "");
    assert_eq!(Nobody.secret("a secret?"), "");
}
