use super::{service, SERVICE};

#[test]
fn a_stack_without_the_gate_runs_none() {
    assert!(service(&[]).is_none());
    assert_eq!(SERVICE, "request-gate");
}

/// A media server's route holds its pins, in its own API or in Emby's.
#[test]
fn only_a_media_servers_route_holds_pins() {
    use crate::ports::service::Target;
    let pins = vec!["a-pin".to_owned()];
    assert_eq!(
        super::media_pins(&Target::Emby { pins: pins.clone() }),
        Some(pins.as_slice())
    );
    assert_eq!(
        super::media_pins(&Target::MediaServer { pins: pins.clone() }),
        Some(pins.as_slice())
    );
}
