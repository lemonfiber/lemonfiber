//! A file left exactly as it was written is known to be current from how it stands.

use super::{balanced, read, scratch, STACKLET};
use crate::app::materialise::materialise;
use crate::stack::Source;

/// Put `text` in `path`, leaving it the size it was and dated when it was.
fn swapped_in_place(path: &std::path::Path, text: &str) {
    let modified = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok();
    let _ = std::fs::write(path, text);
    if let (Some(modified), Ok(file)) = (modified, std::fs::File::options().write(true).open(path))
    {
        let _ = file.set_modified(modified);
    }
}

/// A file standing as it was written is current without a read, and one written
/// since is read and its edit reported — even where the write kept the file's size
/// and put its old date back.
#[test]
fn a_file_written_since_is_read_however_it_is_dated() {
    let (into, record) = scratch("freshness");
    let source = Source::Embedded(&STACKLET);
    let _ = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[]);
    let file = into.join("stack.toml");
    let written = read(&file);

    let untouched = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[])
        .map(|(_, edits)| edits.len());
    assert_eq!(untouched.ok(), Some(0), "standing as written");

    let same_size: String = written.chars().map(|_| 'x').collect();
    swapped_in_place(&file, &same_size);
    let edited = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[])
        .map(|(_, edits)| {
            edits
                .into_iter()
                .map(|edit| edit.path)
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();
    assert_eq!(
        edited,
        vec!["stack.toml".to_owned()],
        "written since, so read"
    );
}
