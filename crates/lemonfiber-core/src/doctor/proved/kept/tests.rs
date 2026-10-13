use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_manifest::container::Posture;

use super::{keep, read, Kept, Key};
use crate::plugin::Verdict;

fn key(image: &str) -> Key {
    Key {
        image: image.to_owned(),
        args: vec!["--version".to_owned()],
        posture: Posture::Confined,
        user: "Uid 1000:1000".to_owned(),
        starting: "sha256:0".to_owned(),
        release: "0.18.0".to_owned(),
    }
}

fn kept(proof: &str, image: &str, reached: &str) -> Kept {
    Kept {
        plugin: "quality".to_owned(),
        proof: proof.to_owned(),
        key: key(image),
        verdict: Verdict::Passed,
        reached: reached.to_owned(),
    }
}

#[test]
fn a_verdict_is_kept_in_place_of_the_last_for_the_same_proof_and_beside_the_others() {
    let at = Scratch::new("kept-verdicts");
    let file = at.join("run-proofs.json");
    keep(Some(&file), read(Some(&file)), kept("a", "old", "1"));
    keep(Some(&file), read(Some(&file)), kept("b", "old", "2"));
    keep(Some(&file), read(Some(&file)), kept("a", "new", "3"));
    let held = read(Some(&file));
    assert_eq!(held, vec![kept("b", "old", "2"), kept("a", "new", "3")]);
}

#[test]
fn nothing_kept_or_nothing_readable_reads_as_nothing_kept() {
    let at = Scratch::new("kept-unreadable");
    assert_eq!(read(None), Vec::new());
    assert_eq!(read(Some(&at.join("absent.json"))), Vec::new());
    let _ = std::fs::write(at.join("garbled.json"), "{not json");
    assert_eq!(read(Some(&at.join("garbled.json"))), Vec::new());
    keep(None, Vec::new(), kept("a", "x", "1"));
}
