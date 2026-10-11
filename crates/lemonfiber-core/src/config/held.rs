use std::fs::File;
use std::path::{Path, PathBuf};

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
        super::store::make_private_dir(parent)?;
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

#[cfg(unix)]
fn opened(at: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(at)
}

#[cfg(not(unix))]
fn opened(at: &Path) -> std::io::Result<File> {
    File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(at)
}
