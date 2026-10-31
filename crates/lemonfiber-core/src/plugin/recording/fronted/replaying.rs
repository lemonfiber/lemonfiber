//! A conformance case that reaches a host outside the stack, which a run holding its
//! recordings to what it takes again leaves as recorded rather than reaching that host:
//! its committed answer stands for it, and later cases' placeholders read it.

use lemonfiber_contract::capabilities::owner::claim;
use std::collections::BTreeMap;

use lemonfiber_contract::conformance::{cases, filed, Case as Asks};
use serde_json::Value;

use super::super::written::answered_before;
use super::speaking::{fixture, named};
use super::{Asking, Case, Faulted, Filling, Reached, Response, ASKED_WITH, JSON};
use crate::plugin::recorded::{Answer, Asked};

/// The operation of `owner.claim` that claims a part.
const CLAIMING: &str = "claim";

impl Asking<'_> {
    /// Every case of each of `contracts` an upstream filing what this one files is asked,
    /// in order: each asked of the adapter, or, where the run holds recordings to what it
    /// takes again and the case reaches outside the stack, as recorded.
    pub(super) async fn every_case(
        &self,
        reached: &Reached,
        filling: &mut Filling<'_>,
        contracts: &[(String, lemonfiber_contract::Capability)],
        heard: &BTreeMap<String, Response>,
    ) -> Result<Vec<Case>, Faulted> {
        let mut asked = Vec::new();
        for (spoken, capability) in contracts {
            for case in filed(capability, &self.upstream.media_types) {
                if let Some(host) = self.left(spoken, &case.case) {
                    asked.push(self.as_recorded(filling, spoken, &case, host));
                    continue;
                }
                let step = named(spoken, &case.case);
                let one = self
                    .case(reached, filling, spoken, &case, heard)
                    .await
                    .map_err(|why| Faulted::new(&step, why, filling.made.clone()))?;
                asked.push(one);
            }
        }
        Ok(asked)
    }

    /// Where the run holds its recordings to what it takes again, the host outside the
    /// stack the case `case` of `spoken` reaches, as the adapter names it.
    pub(super) fn left(&self, spoken: &str, case: &str) -> Option<String> {
        if !self.recorder.taking.check {
            return None;
        }
        self.adapter
            .service
            .outside
            .get(&format!("{spoken}/{case}"))
            .cloned()
    }

    /// Whether the claim asked before any case is left as recorded: the case recording
    /// what claiming answers reaches a host outside the stack.
    pub(super) fn claim_left(&self) -> bool {
        let spoken = lemonfiber_contract::spoken(claim::CAPABILITY, claim::MAJOR);
        lemonfiber_contract::capabilities::all()
            .iter()
            .filter(|capability| capability.name == claim::CAPABILITY)
            .flat_map(cases)
            .filter(|case| case.keyed && case.operation.name == CLAIMING)
            .any(|case| self.left(&spoken, &case.case).is_some())
    }

    /// `case` of `spoken` as its recording holds it, left as recorded because it reaches
    /// `host`; an answer of nothing where no recorded answer is there, which the run refuses.
    pub(super) fn as_recorded(
        &self,
        filling: &mut Filling<'_>,
        spoken: &str,
        case: &Asks<'_>,
        host: String,
    ) -> Case {
        let answer = answered_before(self.root, &fixture(spoken, &case.case)).unwrap_or_default();
        filling.answers.insert(
            format!("{spoken}/{}", case.case),
            answer.json.clone().unwrap_or(Value::Null),
        );
        Case {
            contract: spoken.to_owned(),
            called: case.case.clone(),
            asked: Asked {
                method: ASKED_WITH.to_owned(),
                path: case.operation.path(),
                accept: Some(JSON.to_owned()),
            },
            answer: Answer::clone(&answer),
            left: Some(host),
        }
    }
}
