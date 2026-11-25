use super::reading;

#[test]
fn each_limit_reads_as_the_words_for_it() {
    assert_eq!(reading(None), "anything");
    assert_eq!(reading(Some(0)), "only what suits everyone");
    assert_eq!(reading(Some(15)), "nothing above about 15");
    assert_eq!(reading(Some(13)), "nothing above about 13");
}
