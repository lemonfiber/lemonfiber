use super::{bits, telling};
use crate::ports::service::Occasion;

#[test]
fn each_occasion_is_its_own_bit() {
    let field: Vec<u32> = Occasion::ALL
        .into_iter()
        .map(|occasion| bits(&[occasion].into()))
        .collect();
    assert_eq!(field, [2, 4, 8, 16, 64, 128]);
    assert_eq!(bits(&Occasion::ALL.into()), 222);
}

#[test]
fn a_field_reads_as_the_occasions_it_names_and_whether_it_holds_others() {
    let every = telling(true, 222);
    assert_eq!(every.occasions, Occasion::ALL.into());
    assert!(!every.others);
    let unnamed = telling(false, 256);
    assert!(unnamed.occasions.is_empty());
    assert!(unnamed.others);
    let both = telling(true, 256 | 8);
    assert_eq!(both.occasions, [Occasion::Arrived].into());
    assert!(both.others);
}
