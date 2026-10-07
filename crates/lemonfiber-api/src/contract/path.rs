/// Where the generated contract is kept, relative to the workspace root.
///
/// Its own file so the binary's build script can `include!` it and compare the web
/// app against the same artefact this crate writes, rather than naming it again.
pub const CONTRACT_DIR: &str = "contract/web-api";

/// The file in that directory carrying the wire version and naming every other file.
pub const INDEX: &str = "index.json";
