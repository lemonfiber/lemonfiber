use super::passes;

#[test]
fn a_health_path_reads_up_on_an_answer_or_a_refusal_of_nobody_and_on_nothing_else() {
    for status in [200, 204, 301, 399, 401, 403] {
        assert!(passes(status), "{status}");
    }
    for status in [100, 199, 400, 402, 404, 407, 429, 500, 502, 503] {
        assert!(!passes(status), "{status}");
    }
}
