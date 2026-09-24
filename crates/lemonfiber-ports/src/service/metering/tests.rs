use super::Moved;

#[test]
fn a_count_that_is_only_since_the_last_restart_is_not_the_same_figure() {
    // The direction of the error has to be visible, or the report is one an
    // operator cannot act on.
    let month = Moved {
        down: 100,
        up: 10,
        since_start: false,
    };
    assert_ne!(
        month,
        Moved {
            since_start: true,
            ..month
        }
    );
    assert_eq!(Moved::default().down, 0);
}
