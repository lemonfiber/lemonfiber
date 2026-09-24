use super::Record;

/// Four kinds, four paths. The three libraries are three different words for the
/// same idea across the three services, and getting one wrong would carry television
/// into the films.
#[test]
fn each_kind_of_record_names_its_own_path_and_its_own_word() {
    assert_eq!(Record::Indexer.path(), "/indexer");
    assert_eq!(Record::Series.path(), "/series");
    assert_eq!(Record::Film.path(), "/movie");
    assert_eq!(Record::Artist.path(), "/artist");

    assert_eq!(Record::Indexer.plural(), "indexers");
    assert_eq!(Record::Series.plural(), "series");
    assert_eq!(Record::Film.plural(), "films");
    assert_eq!(Record::Artist.plural(), "artists");
}
