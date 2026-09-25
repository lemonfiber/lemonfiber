use super::{Capacity, Source, Standing, GOES_STALE_AFTER};

/// A moment every case here reads against.
const NOW: u64 = 1_790_812_800;

/// A reading of a ten-megabyte line, taken now.
fn measured(source: Source) -> Capacity {
    Capacity {
        down: 10 * 1024 * 1024,
        up: 1024 * 1024,
        source,
        taken: NOW,
        through_tunnel: false,
    }
}

#[test]
fn a_reading_taken_today_is_one_to_set_a_share_against() {
    assert_eq!(measured(Source::Observed).standing(NOW), Standing::Fresh);
    assert!(measured(Source::Observed).cautions(NOW).is_empty());
}

#[test]
fn a_reading_that_has_stood_too_long_says_how_long() {
    let old = Capacity {
        taken: NOW - GOES_STALE_AFTER - 24 * 60 * 60,
        ..measured(Source::Observed)
    };
    assert_eq!(old.standing(NOW), Standing::Stale(31));
    let said = old.cautions(NOW);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said.first().is_some_and(|line| line.contains("31 days")),
        "{said:?}"
    );
}

#[test]
fn a_reading_taken_through_the_tunnel_says_it_is_the_tunnels() {
    // A share of the tunnel is a smaller limit than the operator asked for,
    // and finding that out from the throughput is finding it out too late.
    let tunnelled = Capacity {
        through_tunnel: true,
        ..measured(Source::Observed)
    };
    let said = tunnelled.cautions(NOW);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said.first().is_some_and(|line| line.contains("tunnel")),
        "{said:?}"
    );
}

#[test]
fn a_clock_that_reads_before_the_measurement_does_not_age_it() {
    // Neither a fault nor an ancient reading: a machine whose clock went
    // backwards has a measurement from its own future, and calling that
    // stale would be reporting the clock as a network problem.
    assert_eq!(measured(Source::Observed).standing(0), Standing::Fresh);
}

#[test]
fn an_observed_figure_is_raised_by_a_better_one_and_never_lowered() {
    let seen = Capacity {
        down: 20 * 1024 * 1024,
        up: 512 * 1024,
        taken: NOW + 60,
        ..measured(Source::Observed)
    };
    let raised = measured(Source::Observed).raised_by(seen);
    assert_eq!(raised.down, 20 * 1024 * 1024, "the better reading wins");
    assert_eq!(raised.up, 1024 * 1024, "and the worse one does not");
    assert_eq!(raised.taken, NOW + 60);
}

#[test]
fn a_figure_the_operator_declared_is_never_overwritten_by_an_observation() {
    let seen = Capacity {
        down: 1,
        up: 1,
        source: Source::Observed,
        taken: NOW + 60,
        through_tunnel: true,
    };
    let kept = measured(Source::Declared).raised_by(seen);
    assert_eq!(kept, measured(Source::Declared));
}

#[test]
fn where_a_figure_came_from_is_said_in_words() {
    assert!(Source::Declared.means().contains("you told"));
    assert!(Source::Observed.means().contains("achieved"));
}
