//! One step's call made concrete: where it goes, and every value put where the recipe
//! wrote it, each held to where it may go.
//!
//! **Where it goes is read, never worked out.** A service in the stack is reached on
//! this machine at the port it publishes; anything else is a host outside it, by the
//! name the manifest writes. The address is built by
//! [`lemonfiber_plugin::addressing::address`], the function reading the manifest built
//! it with, so a call goes exactly where the check that passed it said it would, and
//! the address is held to its destination again as it is built.
//!
//! **A value goes only where it may.** Before anything is built, every value the call
//! would carry is held to its destination ([`super::bounding`]), and a call carrying one
//! anywhere else is not made.
//!
//! **Substitution is the whole of what a value can do.** Each `{{name}}` in a header's
//! value, the body or a query value is replaced by what that name holds, a query value
//! percent-encoded so a value can never close the parameter it stands in.

use std::collections::BTreeMap;

use lemonfiber_plugin::addressing::{address, carries, pieces, placed, Piece, Toward, Unaddressed};
use lemonfiber_plugin::addressing::{CLOSES, OPENS};
use lemonfiber_plugin::StepCall;

use crate::ports::http::{Method, Request};

use super::bounding::Bounds;

/// Where a step's call goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Whither {
    /// A service in the stack, at the port it publishes on this machine.
    Stack(u16),
    /// A host outside it.
    Outside,
}

/// A step's call, ready to send, and every value it carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Built {
    /// What is sent.
    pub(super) request: Request,
    /// Every value it carries, by name.
    pub(super) carried: BTreeMap<String, String>,
}

/// Why a step's call was not made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Unbuilt {
    /// It calls with a method lemonfiber does not send.
    Method,
    /// Its address could not be built as the manifest writes it, and why.
    Unaddressed(String),
    /// A value it carries may not go where it is going, and why.
    Withheld(String),
}

/// The request one step makes, every substitution put in.
///
/// # Errors
///
/// Where the method is one the port cannot carry, a value it would carry may not go to
/// its destination, or its address is not built as the manifest writes it.
pub(super) fn request(
    call: &StepCall,
    whither: Whither,
    values: &BTreeMap<String, String>,
    bounds: &Bounds<'_>,
) -> Result<Built, Unbuilt> {
    let method: Method = crate::plugin::judging::method(&call.method).ok_or(Unbuilt::Method)?;
    let carried: BTreeMap<String, String> = placed(call)
        .into_iter()
        .filter_map(|(_, name)| values.get_key_value(name))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    let outside = whither == Whither::Outside;
    if let Some(why) = carried
        .iter()
        .find_map(|(name, value)| bounds.withheld(name, value, &call.to, outside))
    {
        return Err(Unbuilt::Withheld(why));
    }
    let url = address(&call.path, toward(whither, &call.to), &|written| {
        substituted(written, values, true)
    })
    .map_err(|unaddressed| Unbuilt::Unaddressed(said(unaddressed)))?;
    let headers = call
        .headers
        .iter()
        .flatten()
        .map(|(name, value)| (name.clone(), substituted(value, values, false)))
        .collect();
    let request = Request {
        method,
        url: url.to_string(),
        headers,
        body: call
            .body
            .as_deref()
            .map(|body| substituted(body, values, false)),
        pinned: None,
    };
    Ok(Built { request, carried })
}

/// Where a step's call is addressed, as [`address`] builds it.
fn toward(whither: Whither, to: &str) -> Toward<'_> {
    match whither {
        Whither::Stack(port) => Toward::Stack(port),
        Whither::Outside => Toward::Outside(to),
    }
}

/// The request about to be sent, held once more to its destination.
///
/// Asked of what is handed to the transport, after everything between building it and
/// sending it, so nothing in between can move it.
///
/// # Errors
///
/// Where its address goes anywhere but exactly toward `to`, saying where it goes.
pub(super) fn sending(request: Request, whither: Whither, to: &str) -> Result<Request, String> {
    if carries(&request.url, toward(whither, to)) {
        return Ok(request);
    }
    Err(format!(
        "was not sent: its address {} does not go to {to}",
        request.url
    ))
}

/// Why an address was not built, as a step says it.
fn said(unaddressed: Unaddressed) -> String {
    match unaddressed {
        Unaddressed::Path(why) => format!("its path is not a plain absolute path: {why}"),
        Unaddressed::Host(why) => why,
    }
}

/// `text` with each `{{name}}` replaced by what that name holds, encoded for a query
/// value where `encoding`.
///
/// A name nothing holds is left as it was written. Reading the manifest refused every
/// substitution of a value no earlier step captures and no input brings in, so the one
/// way to meet one here is a capture the step that makes it skipped — and a call that
/// carries the braces says so to whoever reads it, where an empty string would not.
fn substituted(text: &str, values: &BTreeMap<String, String>, encoding: bool) -> String {
    pieces(text)
        .into_iter()
        .map(|piece| match piece {
            Piece::Written(written) => written.to_owned(),
            Piece::Named(name) => match values.get(name.trim()) {
                Some(value) if encoding => encoded(value),
                Some(value) => value.clone(),
                None => format!("{OPENS}{name}{CLOSES}"),
            },
        })
        .collect()
}

/// A value as a query carries it: every byte but the unreserved ones percent-encoded.
fn encoded(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
