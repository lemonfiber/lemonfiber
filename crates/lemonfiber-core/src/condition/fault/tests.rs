use super::Fault;
use crate::error::Severity;

#[test]
fn a_fault_cannot_be_built_without_something_to_do_about_it() {
    // Not enforced by a check at runtime but by the constructor: there is no way
    // to reach a fault with an empty remedy list.
    let fault = Fault::new(
        "storage.full",
        Severity::Error,
        "the disk is full",
        "nothing can be written until something goes",
        "delete something",
    );
    assert_eq!(fault.remedies, vec!["delete something".to_owned()]);
    assert_eq!(fault.caused_by, None);
    assert_eq!(fault.kind, "storage.full");
}

#[test]
fn a_fault_says_what_it_costs_as_well_as_what_happened_and_what_to_do() {
    // The middle part, and the one that is quietly dropped: an event and an
    // instruction leave whoever reads them to work out for themselves whether
    // this is worth getting up for.
    let fault = Fault::new(
        "vpn.egress.leaking",
        Severity::Critical,
        "the download client's traffic is not going through the tunnel",
        "every peer it talks to can see this connection's own address",
        "stop the download client",
    );
    assert_eq!(
        fault.meaning,
        "every peer it talks to can see this connection's own address"
    );
    assert!(!fault.summary.is_empty());
    assert!(!fault.remedies.is_empty());
}

#[test]
fn what_a_fault_means_is_withheld_on_the_same_rules_as_the_rest_of_it() {
    // The consequence of a failed login is the obvious place to quote the
    // credential that failed, and a sentence nobody thought of as evidence is
    // the one that reaches a phone unredacted.
    let quoted = format!("nothing will authenticate while {}=hunter2 stands", "TOKEN");
    let fault = Fault::new(
        "service.refused",
        Severity::Error,
        "the login failed",
        &quoted,
        "check the key",
    );
    assert!(!fault.meaning.contains("hunter2"), "{}", fault.meaning);
    assert!(fault.meaning.contains("nothing will authenticate"));
}

#[test]
fn further_remedies_keep_the_order_they_were_offered() {
    // Most likely first, as everywhere else remedies are listed.
    let fault = Fault::new(
        "storage.full",
        Severity::Error,
        "the disk is full",
        "nothing can be written until something goes",
        "delete something",
    )
    .or_else("move the library to a larger volume");
    assert_eq!(
        fault.remedies,
        vec![
            "delete something".to_owned(),
            "move the library to a larger volume".to_owned()
        ]
    );
}

#[test]
fn a_fault_can_name_the_one_it_is_downstream_of() {
    let fault = Fault::new(
        "import.failed",
        Severity::Error,
        "the import failed",
        "it stays out of the library until it lands",
        "retry the import",
    )
    .caused_by("storage.space");
    assert_eq!(fault.caused_by.as_deref(), Some("storage.space"));
}

#[test]
fn a_credential_a_service_repeated_back_never_reaches_a_fault() {
    // A service that fails while authenticating says so with the credential in
    // hand, and that message becomes a summary, a stored condition, a digest,
    // and a push to somebody's phone.
    let leaked = format!("sonarr refused: {}=abcdef123456", "api_key");
    let fault = Fault::new(
        "service.refused",
        Severity::Error,
        &leaked,
        "nothing that needs it is working",
        "check the key",
    );
    assert!(!fault.summary.contains("abcdef123456"), "{}", fault.summary);
    assert!(
        fault.summary.contains("sonarr refused"),
        "{}",
        fault.summary
    );
}

#[test]
fn a_remedy_is_redacted_on_the_same_rules_as_the_summary() {
    // A remedy that quotes the offending line is the obvious way for one to get
    // out, and the least obvious place to look for it.
    let quoted = format!("set {}=hunter2 in the environment file", "PASSWORD");
    let fault = Fault::new(
        "config.wrong",
        Severity::Error,
        "the login failed",
        "the service will not answer until it is right",
        &quoted,
    )
    .or_else(&quoted);
    assert!(
        fault
            .remedies
            .iter()
            .all(|remedy| !remedy.contains("hunter2")),
        "{:?}",
        fault.remedies
    );
}

#[test]
fn a_credential_written_after_a_colon_is_withheld_too() {
    // The other shape a service quotes one back in. Both are ordinary; catching
    // only the one with an equals sign would be a rule that holds until the
    // next service words its error differently.
    let leaked = format!("sonarr refused: {}: abcdef123456", "api_key");
    let fault = Fault::new(
        "service.refused",
        Severity::Error,
        &leaked,
        "nothing that needs it is working",
        "check the key",
    );
    assert!(!fault.summary.contains("abcdef123456"), "{}", fault.summary);
    assert!(fault.summary.contains("api_key"), "{}", fault.summary);
}

#[test]
fn wording_that_carries_no_credential_is_left_exactly_as_written() {
    // Redaction that mangled ordinary sentences would be its own problem.
    let plain = "sonarr keeps restarting";
    let fault = Fault::new(
        "service.crash-looping",
        Severity::Error,
        plain,
        "nothing that needs it is working",
        "read its logs",
    );
    assert_eq!(fault.summary, plain);
    assert_eq!(fault.remedies, vec!["read its logs".to_owned()]);
}
