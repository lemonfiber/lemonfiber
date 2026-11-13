use std::fs::File;
use std::path::{Path, PathBuf};

use lemonfiber_platform::fs;

const SUFFIX: &str = ".lock";

#[derive(Debug)]
pub(crate) struct Hold(File);

impl Hold {
    pub(crate) fn taken(path: &Path) -> std::io::Result<Self> {
        let at = beside(path);
        let parent = at
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::private_directory(parent, true)?;
        let file = opened(&at)?;
        file.lock()?;
        Ok(Self(file))
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

pub(crate) fn beside(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(SUFFIX);
    path.with_file_name(name)
}

fn opened(at: &Path) -> std::io::Result<File> {
    let mut options = File::options();
    options.read(true).write(true).create(true).truncate(false);
    fs::owner_only(&mut options);
    fs::unfollowed_unwaiting(&mut options).open(at)
}
