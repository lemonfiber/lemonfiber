use super::{Limit, Resolved, DOWNLOAD_SHARE, UPLOAD_SHARE};

/// Ten megabytes a second down, which is an ordinary household line.
const LINE: u64 = 10 * 1024 * 1024;

#[test]
fn a_limit_can_be_a_proportion_as_well_as_a_figure() {
    assert_eq!(Limit::read("50%"), Some(Limit::Share(50)));
    assert_eq!(
        Limit::read(" 2 MiB "),
        Some(Limit::Absolute(2 * 1024 * 1024))
    );
    assert_eq!(Limit::read("unlimited"), Some(Limit::Unlimited));
    assert_eq!(Limit::read("None"), Some(Limit::Unlimited));
    assert_eq!(Limit::read("off"), Some(Limit::Unlimited));
}

#[test]
fn a_proportion_of_the_line_becomes_the_figure_the_client_is_given() {
    assert_eq!(Limit::Share(50).against(Some(LINE)), Resolved::At(LINE / 2));
    assert_eq!(
        Limit::Absolute(1_000).against(Some(LINE)),
        Resolved::At(1_000)
    );
    assert_eq!(Limit::Unlimited.against(Some(LINE)), Resolved::Unlimited);
}

#[test]
fn a_share_of_a_line_nobody_measured_is_its_own_answer() {
    // Not unlimited. "Half of an unknown number" that resolves to "no limit"
    // is a setting the operator believes is in force while the stack takes
    // the whole line, which is the failure this feature exists to stop.
    assert_eq!(Limit::Share(50).against(None), Resolved::Unmeasured);
    assert_ne!(Limit::Share(50).against(None), Resolved::Unlimited);
    // A line measured at nothing is no measurement rather than a measurement
    // of nothing. Resolving it would give a limit of zero bytes a second,
    // which is not an unlimited client but a stopped one.
    assert_eq!(Limit::Share(50).against(Some(0)), Resolved::Unmeasured);
    assert_eq!(Limit::Absolute(1_000).against(None), Resolved::At(1_000));
    assert_eq!(Limit::Unlimited.against(None), Resolved::Unlimited);
}

#[test]
fn a_proportion_is_never_shown_without_the_line_it_is_a_proportion_of() {
    let said = Limit::Share(50).says(Some(LINE));
    assert!(said.contains("50%"), "{said}");
    assert!(said.contains("10.0 MiB/s"), "the measured line: {said}");
    assert!(said.contains("5.0 MiB/s"), "and what it comes to: {said}");
}

#[test]
fn a_proportion_of_nothing_measured_says_that_it_holds_nothing_back() {
    let said = Limit::Share(50).says(None);
    assert!(said.contains("nothing has measured"), "{said}");
    assert!(said.contains("nothing is held back"), "{said}");
}

#[test]
fn an_absolute_limit_reads_as_the_figure_it_is() {
    assert_eq!(Limit::Absolute(LINE).says(None), "10.0 MiB/s");
    assert_eq!(Limit::Absolute(LINE).written(), "10.0 MiB/s");
    assert_eq!(Limit::Unlimited.says(Some(LINE)), "no limit");
    assert_eq!(Limit::Unlimited.written(), "unlimited");
    assert_eq!(Limit::Share(25).written(), "25%");
}

#[test]
fn a_limit_that_could_not_be_read_is_refused_rather_than_rounded() {
    assert_eq!(Limit::read("half"), None);
    assert_eq!(Limit::read("0%"), None);
    assert_eq!(Limit::read("101%"), None);
    assert_eq!(Limit::read("-5%"), None);
    assert_eq!(Limit::read("0"), None, "a limit of nothing is not a limit");
    assert_eq!(Limit::read("5 furlongs"), None);
}

#[test]
fn the_upload_default_is_more_conservative_than_the_download_one() {
    // The requirement, held as a rule rather than as two numbers somebody
    // remembers to keep in order. A saturated uplink degrades downloads too,
    // because acknowledgements cannot get out past the queue of upload data.
    // Read through what each share comes to against one line, rather than
    // compared as two numbers. `UPLOAD_SHARE < DOWNLOAD_SHARE` is settled at
    // compile time and asserts nothing at run time: it would hold just as well
    // against an `against` that ignored both shares and handed back the whole
    // line twice. What being the more careful default comes to is less room on
    // the same connection, which is a figure and can be compared.
    let careful = LINE * u64::from(UPLOAD_SHARE) / 100;
    let generous = LINE * u64::from(DOWNLOAD_SHARE) / 100;
    assert_eq!(
        Limit::Share(UPLOAD_SHARE).against(Some(LINE)),
        Resolved::At(careful)
    );
    assert_eq!(
        Limit::Share(DOWNLOAD_SHARE).against(Some(LINE)),
        Resolved::At(generous)
    );
    assert!(
        careful < generous,
        "the upload default came to {careful} against the download's {generous}"
    );
}

#[test]
fn only_a_proportion_owes_the_measured_figure() {
    assert!(Limit::Share(50).is_share());
    assert!(!Limit::Absolute(1).is_share());
    assert!(!Limit::Unlimited.is_share());
}
