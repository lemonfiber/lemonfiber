use super::Trouble;

#[test]
fn a_connection_complaint_is_read_as_a_crowded_account() {
    assert_eq!(
        Trouble::of("Too many connections to server news.example.com [502 Too many connections]"),
        Trouble::Crowded
    );
    assert_eq!(
        Trouble::of("481 Connection limit exceeded for your account"),
        Trouble::Crowded
    );
}

/// The distinction the whole ordering exists for: both sentences complain about a
/// limit being exceeded, and only one of them is about connections.
#[test]
fn a_data_limit_is_not_read_as_a_connection_limit() {
    assert_eq!(Trouble::of("502 Download limit exceeded"), Trouble::Spent);
    assert_eq!(
        Trouble::of("502 You have exceeded your byte allowance"),
        Trouble::Spent
    );
}

#[test]
fn a_rejected_credential_is_read_as_a_refusal() {
    assert_eq!(
        Trouble::of("Failed login for server news.example.com [481 Authentication failed]"),
        Trouble::Refused
    );
    assert_eq!(
        Trouble::of("502 Invalid username or password"),
        Trouble::Refused
    );
}

#[test]
fn an_account_out_of_credit_is_read_as_spent() {
    assert_eq!(
        Trouble::of("502 No credits left on this account"),
        Trouble::Spent
    );
    assert_eq!(
        Trouble::of("502 Your subscription has expired"),
        Trouble::Spent
    );
}

/// The default matters more than the matches: anything unrecognised must not become
/// a rejected credential, which is the one reading that sends an operator to change
/// something that was never wrong.
#[test]
fn words_nothing_places_stay_unplaced() {
    assert_eq!(Trouble::of(""), Trouble::Unplaced);
    assert_eq!(
        Trouble::of("Cannot connect to server news.example.com [timed out]"),
        Trouble::Unplaced
    );
}
