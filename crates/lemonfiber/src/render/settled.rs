//! What setup settled, and whether this stack is set up.

use lemonfiber_core::model::{SetUp, SettledReport, UserFrom};

use super::Lines;

/// What setup settled, and whether this stack is set up.
pub(super) fn settled(report: &SettledReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(match report.setup {
        SetUp::None => "This stack is not set up.",
        SetUp::PartWay => "Setup is part-way through.",
        SetUp::Done => "This stack is set up.",
    });
    lines.put(format!(
        "  Data root     {}",
        report.data_root.as_deref().unwrap_or("not chosen")
    ));
    lines.put(format!(
        "  Downloading   {}",
        protocols(report.protocols.usenet, report.protocols.torrent)
    ));
    lines.put(format!(
        "  Services run  as {}:{}, {}",
        report.service_user.uid,
        report.service_user.gid,
        from(report.service_user.from)
    ));
    lines
}

/// The ways of downloading that are configured, in words.
const fn protocols(usenet: bool, torrent: bool) -> &'static str {
    match (usenet, torrent) {
        (true, true) => "Usenet and torrents",
        (true, false) => "Usenet",
        (false, true) => "torrents",
        (false, false) => "none configured",
    }
}

/// Where the user the services run as comes from, in words.
const fn from(from: UserFrom) -> &'static str {
    match from {
        UserFrom::Set => "as PUID and PGID set them",
        UserFrom::PartSet => "one of PUID and PGID set, the other the stack directory's owner's",
        UserFrom::StackOwner => "the stack directory's owner, as neither PUID nor PGID is set",
    }
}

#[cfg(test)]
mod tests;
