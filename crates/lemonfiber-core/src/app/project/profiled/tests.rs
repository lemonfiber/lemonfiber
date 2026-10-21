use std::collections::BTreeSet;

use super::read;

#[test]
fn the_profiles_the_file_names_are_read_back_and_nothing_else() {
    let read = read("OTHER=x\nCOMPOSE_PROFILES=media, dl,,plugin-komga\nCOMPOSE_PROFILESX=no\n");

    assert_eq!(
        read,
        ["dl", "media", "plugin-komga"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<String>>()
    );
}
