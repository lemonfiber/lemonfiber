use super::{Queue, QueueDepth, Queued};

/// One queue item at a tracked status.
fn item(title: &str, status: &str) -> Queued {
    Queued {
        title: title.to_owned(),
        status: status.to_owned(),
        state: "downloading".to_owned(),
        message: None,
        download_id: None,
        grabs: 1,
    }
}

#[test]
fn a_queue_deeper_than_one_page_reports_its_true_depth() {
    // Counting the depth from the page would silently report 200 for a queue
    // of 500. The service's own total is authoritative; only the stuck count
    // comes from what came back, which under-counts rather than invents.
    let read = Queue {
        total: 500,
        items: vec![item("a", "ok"), item("b", "warning")],
    };
    assert_eq!(
        QueueDepth::of(&read),
        QueueDepth {
            total: 500,
            stuck: 1
        }
    );
}

#[test]
fn the_services_own_words_decide_what_is_stuck_however_they_are_cased() {
    assert!(item("a", "warning").is_stuck());
    assert!(item("a", "Error").is_stuck());
    assert!(!item("a", "ok").is_stuck());
    assert!(!item("a", String::new().as_str()).is_stuck());
}

#[test]
fn an_empty_queue_is_a_working_stack_with_nothing_to_do() {
    // Distinct from one that could not be read, which never reaches here — it
    // is a `Failure`. Rendering the two alike is how an operator comes to
    // believe a service is idle when it is unreachable.
    assert!(Queue::default().is_empty());
    assert!(!Queue {
        total: 1,
        items: Vec::new()
    }
    .is_empty());
    assert!(!Queue {
        total: 0,
        items: vec![item("a", "ok")]
    }
    .is_empty());
}
