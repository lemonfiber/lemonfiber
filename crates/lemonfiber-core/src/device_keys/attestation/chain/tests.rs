use rcgen::{BasicConstraints, CustomExtension};

use super::{read, verified, Dated};
use crate::device_keys::testing::{arcs, now, pair, sequence, tlv, Authority};

/// The chain `certificates`, read and checked against `roots` at the test moment.
fn holds(certificates: &[Vec<u8>], roots: &[&[u8]]) -> bool {
    read(certificates).is_some_and(|read| verified(&read, roots, now(), Dated::Every))
}

#[test]
fn a_key_certified_by_an_authority_the_root_signed_reads_back_to_it() {
    let root = Authority::root("root");
    let authority = root.below("authority", Some(BasicConstraints::Unconstrained));
    let own = authority.certify("own", &pair(), Vec::new());
    assert!(holds(&[own, authority.der.clone()], &[&root.der]));
}

#[test]
fn a_chain_ending_in_the_root_itself_reads_back_to_it() {
    let root = Authority::root("root");
    let own = root.certify("own", &pair(), Vec::new());
    assert!(holds(&[own, root.der.clone()], &[&root.der]));
}

#[test]
fn a_certificate_carrying_a_roots_key_without_its_signature_anchors_nothing() {
    let root = Authority::root("root");
    let forger = Authority::root("root");
    let own = forger.certify("own", &pair(), Vec::new());
    assert!(!holds(&[own, forger.der.clone()], &[&root.der]));
}

#[test]
fn a_key_certificate_alone_is_never_its_own_anchor() {
    let root = Authority::root("root");
    let own = root.certify("own", &pair(), Vec::new());
    assert!(!holds(&[own], &[&root.der]));
    assert!(!holds(std::slice::from_ref(&root.der), &[&root.der]));
}

#[test]
fn a_chain_missing_its_intermediate_is_refused() {
    let root = Authority::root("root");
    let authority = root.below("authority", Some(BasicConstraints::Unconstrained));
    let own = authority.certify("own", &pair(), Vec::new());
    assert!(!holds(&[own.clone(), root.der.clone()], &[&root.der]));
    assert!(!holds(&[own], &[&root.der]));
}

#[test]
fn an_intermediate_that_is_no_authority_signs_nothing() {
    let root = Authority::root("root");
    let impostor = root.below("impostor", None);
    let own = impostor.certify("own", &pair(), Vec::new());
    assert!(!holds(&[own, impostor.der.clone()], &[&root.der]));
}

#[test]
fn an_authority_held_to_no_authorities_below_it_signs_no_authority() {
    let root = Authority::root("root");
    let narrow = root.below("narrow", Some(BasicConstraints::Constrained(0)));
    let under = narrow.below("under", Some(BasicConstraints::Unconstrained));
    let own = under.certify("own", &pair(), Vec::new());
    assert!(!holds(
        &[own, under.der.clone(), narrow.der.clone()],
        &[&root.der]
    ));
    let direct = narrow.certify("own", &pair(), Vec::new());
    assert!(holds(&[direct, narrow.der.clone()], &[&root.der]));
}

#[test]
fn a_chain_to_another_root_is_refused() {
    let root = Authority::root("root");
    let other = Authority::root("other");
    let own = other.certify("own", &pair(), Vec::new());
    assert!(!holds(&[own, other.der.clone()], &[&root.der]));
}

#[test]
fn certificates_out_of_date_are_refused_and_the_keys_own_dates_can_be_set_aside() {
    let root = Authority::root("root");
    let own = root.certify("own", &pair(), Vec::new());
    let chain = [own, root.der.clone()];
    let Some(read) = read(&chain) else {
        return;
    };
    let long_ago = std::time::UNIX_EPOCH;
    assert!(!verified(&read, &[&root.der], long_ago, Dated::Every));
    assert!(!verified(&read, &[&root.der], long_ago, Dated::ButTheKeys));
    assert!(verified(&read, &[&root.der], now(), Dated::ButTheKeys));
}

#[test]
fn nothing_that_is_not_a_short_chain_of_certificates_reads() {
    assert!(read(&[]).is_none());
    assert!(read(&[b"not a certificate".to_vec()]).is_none());
    let root = Authority::root("root");
    let mut trailing = root.der.clone();
    trailing.push(0);
    assert!(read(&[trailing]).is_none());
    assert!(read(&vec![root.der.clone(); 11]).is_none());
}

#[test]
fn a_certificate_carrying_a_critical_extension_nothing_here_reads_is_refused() {
    let root = Authority::root("root");
    let mut extension = CustomExtension::from_oid_content(&[1, 2, 3, 4], vec![0x05, 0x00]);
    extension.set_criticality(true);
    let own = root.certify("own", &pair(), vec![extension]);
    assert!(!holds(&[own, root.der.clone()], &[&root.der]));
}

#[test]
fn a_certificate_marking_a_constraint_nothing_here_holds_to_critical_is_refused() {
    let root = Authority::root("root");
    let subtree = sequence(&[tlv(&[0x82], b"example.com")]);
    let constraints = [
        (arcs("2.5.29.30"), sequence(&[tlv(&[0xa0], &subtree)])),
        (arcs("2.5.29.36"), sequence(&[tlv(&[0x80], &[0])])),
        (arcs("2.5.29.54"), tlv(&[0x02], &[0])),
    ];
    for (oid, content) in constraints {
        let mut extension = CustomExtension::from_oid_content(&oid, content.clone());
        let own = root.certify("own", &pair(), vec![extension.clone()]);
        assert!(holds(&[own, root.der.clone()], &[&root.der]));
        extension.set_criticality(true);
        let own = root.certify("own", &pair(), vec![extension]);
        assert!(!holds(&[own, root.der.clone()], &[&root.der]));
    }
}

#[test]
fn a_certificate_signed_under_another_algorithm_than_its_signed_part_names_is_refused() {
    let root = Authority::root("root");
    let own = root.certify("own", &pair(), Vec::new());
    assert!(read(std::slice::from_ref(&own)).is_some());
    let sha256 = [0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x02];
    let outer = own
        .windows(sha256.len())
        .rposition(|window| window == sha256)
        .map(|at| at + sha256.len() - 1);
    let mut renamed = own;
    if let Some(byte) = outer.and_then(|at| renamed.get_mut(at)) {
        *byte = 0x03;
    }
    assert!(read(&[renamed]).is_none());
}
