use super::{service, SERVICE};

#[test]
fn a_stack_without_the_gate_runs_none() {
    assert!(service(&[]).is_none());
    assert_eq!(SERVICE, "request-gate");
}
