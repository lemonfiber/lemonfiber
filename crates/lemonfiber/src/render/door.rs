//! The one address to hand somebody who lives here.
//!
//! One of the renderers, its own file so each answer's shape is read on its own.
//! Every one of them builds lines and hands them back; the printer is at the edge.
//!
//! The shape is the answer first and the working underneath it. An operator who
//! only wanted to know what to send has it in the first two lines — what it is
//! called and the address itself — and one who wondered why it was not the page
//! that links everything reads on and finds that question answered by name.

use lemonfiber_core::model::{FrontDoorReport, Standing};

use super::Lines;

/// What the household begins at, and what stands beside it that is not it.
pub(super) fn front_door(report: &FrontDoorReport) -> Lines {
    let mut lines = Lines::default();
    match &report.service {
        Some(service) => lines.put(format!("{service}   {}", standing(report.standing))),
        None => lines.put(standing(report.standing)),
    }
    if let Some(address) = &report.address {
        lines.put(format!("  {}", address.url));
    }
    lines.put(format!("  {}", report.meaning));
    if let Some(caution) = report
        .address
        .as_ref()
        .and_then(|address| address.caution.as_ref())
    {
        lines.put(format!("  {caution}"));
    }

    if !report.beside.is_empty() {
        lines.spaced("Also on your network, and none of them a way in:");
        for beside in &report.beside {
            lines.put(format!("  {}   {}", beside.service, beside.because));
        }
    }
    lines
}

/// Where the door stands, in one phrase.
const fn standing(standing: Standing) -> &'static str {
    match standing {
        Standing::Established => "the front door",
        Standing::LibraryOnly => "the front door, and there is nothing here to ask for",
        Standing::Unreachable => "the front door, and it is not answering",
        Standing::Absent => "There is no front door.",
    }
}

#[cfg(test)]
mod tests {
    use super::{front_door, standing};
    use lemonfiber_core::door::{Address, Facing};
    use lemonfiber_core::model::{Beside, FrontDoorReport, Standing};

    /// A report naming one service, with one thing beside it.
    fn report(standing: Standing, service: Option<&str>) -> FrontDoorReport {
        FrontDoorReport {
            standing,
            service: service.map(str::to_owned),
            address: service.map(|_| Address {
                url: "http://kitchen-nas.local:5055".to_owned(),
                caution: None,
            }),
            facing: service.map(|_| Facing::Asking),
            meaning: "what this comes to".to_owned(),
            beside: vec![Beside {
                service: "Homepage".to_owned(),
                facing: Facing::Operators,
                because: Facing::Operators.because().to_owned(),
            }],
        }
    }

    #[test]
    fn the_answer_is_the_first_line_and_the_working_is_underneath() {
        let said = front_door(&report(Standing::Established, Some("Seerr"))).text();
        let first = said.lines().next();
        assert_eq!(first, Some("Seerr   the front door"));
        assert!(said.contains("what this comes to"));
    }

    #[test]
    fn the_address_is_the_line_under_the_name() {
        let said = front_door(&report(Standing::Established, Some("Seerr"))).text();
        assert_eq!(said.lines().nth(1), Some("  http://kitchen-nas.local:5055"));
    }

    #[test]
    fn an_address_that_may_change_says_so_under_what_it_means() {
        let mut given = report(Standing::Established, Some("Seerr"));
        given.address = Some(Address {
            url: "http://192.168.1.10:5055".to_owned(),
            caution: Some("it can stop working".to_owned()),
        });
        let said = front_door(&given).text();
        let at = said.lines().position(|line| line.contains("192.168.1.10"));
        let warned = said
            .lines()
            .position(|line| line.contains("it can stop working"));
        assert!(at.is_some_and(|at| warned.is_some_and(|warned| warned > at)));
    }

    #[test]
    fn what_is_not_the_door_is_named_with_the_reason_it_is_not() {
        let said = front_door(&report(Standing::Established, Some("Seerr"))).text();
        assert!(said.contains("none of them a way in"));
        assert!(said.contains("Homepage"));
        assert!(said.contains("never a way in"));
    }

    #[test]
    fn no_door_is_said_outright_rather_than_left_to_an_empty_line() {
        let mut nothing = report(Standing::Absent, None);
        nothing.beside.clear();
        let said = front_door(&nothing).text();
        assert!(said.starts_with("There is no front door."));
        assert!(!said.contains("way in"));
    }

    #[test]
    fn each_standing_reads_differently_from_the_others() {
        let said = [
            Standing::Established,
            Standing::LibraryOnly,
            Standing::Unreachable,
            Standing::Absent,
        ]
        .map(standing);
        let mut unique = said.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), said.len());
    }
}
