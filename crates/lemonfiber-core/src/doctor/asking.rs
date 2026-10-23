//! A question an installed plugin declares of one of its own services, put and judged.
//!
//! One way of asking for every row that asks: a contributed check and a request proof
//! name a service, a request and what the answer must be in one vocabulary, so they are
//! put the same way and judged by the same evaluator a plugin's recordings are.

use std::collections::BTreeMap;

use lemonfiber_plugin::{Expect, Request};

use crate::plugin::judging::{judge, live, method};
use crate::ports::http::{self, Http, Method};

/// What a row asks, or why nothing can be asked.
#[derive(Debug)]
pub(super) enum Asks {
    /// Everything the engine needs to put the question.
    Service {
        /// Where the plugin's service answers.
        address: String,
        /// The method, as the transport carries it.
        method: Method,
        /// The path on that service.
        path: String,
        /// The one representation the answer is asked for, where the row names one.
        accept: Option<String>,
        /// What the answer has to be.
        expect: Box<Expect>,
    },
    /// Why the question cannot be put, in the words the finding carries.
    Nothing(String),
}

/// The question a row puts of `service`, where every part of it is there.
///
/// `answering` says where each of the plugin's services can be reached, by the id the
/// manifest gave it; a service missing from it is one lemonfiber cannot ask.
pub(super) fn asked(
    service: Option<&str>,
    request: Option<&Request>,
    expect: Option<&Expect>,
    answering: &BTreeMap<String, String>,
) -> Asks {
    let Some(service) = service else {
        return Asks::Nothing(
            "it does not settle which of this plugin's services it asks".to_owned(),
        );
    };
    let Some(address) = answering.get(service) else {
        return Asks::Nothing(format!(
            "{service} is not answering anywhere lemonfiber can reach"
        ));
    };
    let (Some(request), Some(expect)) = (request, expect) else {
        return Asks::Nothing(
            "it asks nothing, or says nothing about the answer, so there is nothing to establish"
                .to_owned(),
        );
    };
    let Some(method) = method(&request.method) else {
        return Asks::Nothing(format!(
            "{} is not a method lemonfiber can send",
            request.method
        ));
    };
    if !lemonfiber_plugin::refusing::carried::is_route(&request.path) {
        return Asks::Nothing(format!(
            "{} is not a route on {service}, so it was never asked",
            request.path
        ));
    }
    Asks::Service {
        address: address.clone(),
        method,
        path: request.path.clone(),
        accept: request.accept.clone(),
        expect: Box::new(expect.clone()),
    }
}

/// What putting the question came to: each fault the answer has, none where it holds,
/// or why it could not be put at all.
///
/// No credential goes with it. A row has nowhere to write one, which keeps asking as
/// nobody a property of the format rather than a convention.
pub(super) async fn put(http: &dyn Http, asks: &Asks) -> Result<Vec<String>, String> {
    let (address, method, path, accept, expect) = match asks {
        Asks::Nothing(why) => return Err(why.clone()),
        Asks::Service {
            address,
            method,
            path,
            accept,
            expect,
        } => (address, method, path, accept, expect),
    };
    let headers = accept
        .iter()
        .map(|one| ("Accept".to_owned(), one.clone()))
        .collect();
    let answered = http
        .send(&http::Request {
            method: *method,
            url: format!("{}{path}", address.trim_end_matches('/')),
            headers,
            body: None,
            pinned: None,
        })
        .await;
    match answered {
        Err(unreachable) => Err(format!(
            "{} did not answer: {}",
            unreachable.url, unreachable.reason
        )),
        Ok(response) => Ok(judge(expect, &live(&response))),
    }
}
