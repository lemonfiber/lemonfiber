use std::path::Path;

use lemonfiber_sidecar::gate::File;

use super::{path, service, SERVICE};

#[test]
fn the_gates_files_live_in_its_own_configuration_directory() {
    assert_eq!(
        path(Path::new("/stack"), File::Upstreams),
        Path::new("/stack/config/request-gate/upstreams.json")
    );
}

#[test]
fn a_stack_without_the_gate_runs_none() {
    assert!(service(&[]).is_none());
    assert_eq!(SERVICE, "request-gate");
}
