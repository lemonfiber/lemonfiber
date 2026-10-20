use super::Curated;

#[test]
fn a_curator_written_for_a_reader_withholds_its_key() {
    let curated = Curated {
        media_type: "music".to_owned(),
        address: "http://curator:80".to_owned(),
        key: "the-curators-own-key".to_owned(),
        folder: "/data/downloads/music".to_owned(),
    };
    let written = format!("{curated:?}");
    assert!(!written.contains("the-curators-own-key"), "{written}");
    assert!(written.contains("/data/downloads/music"), "{written}");
}
