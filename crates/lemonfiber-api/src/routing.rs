//! Every route the surface serves, as it is routed and as the contract lists it.
//!
//! A route is declared once, where its module declares it, with what it answers under.
//! The router a run serves and the list the contract publishes are both built by that
//! declaration, so a route the surface gains is a route the list names, and the list
//! cannot name a route nothing serves.

use axum::handler::Handler;
use axum::http::Method;
use axum::routing::{delete, get, post, MethodRouter};
use axum::Router;
use lemonfiber_core::model::kind::Kind;
use serde::{Serialize, Serializer};

/// One route the surface serves, as the contract lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Route {
    /// The method it is served at, spelled as HTTP spells it.
    #[serde(serialize_with = "spelled")]
    pub method: Method,
    /// Its path as the surface routes it, `{name}` and `{job}` left as they are.
    pub path: &'static str,
    /// Whether it takes a body: the one `bodies` describes under its path, or for an
    /// action the arguments `actions` lists.
    pub body: bool,
    /// Every kind its answer may be written under, by name, each once and in order. Empty
    /// for a route that answers with a file.
    pub kinds: Vec<&'static str>,
}

/// Whether a route reads a body from its request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taking {
    /// Nothing beyond its path and its query.
    Nothing,
    /// A JSON body.
    Body,
}

/// Routes, and the list of them, built by the same declarations.
pub struct Serves<S> {
    /// What a run serves.
    router: Router<S>,
    /// What the contract lists.
    routes: Vec<Route>,
}

impl<S: Clone + Send + Sync + 'static> Default for Serves<S> {
    fn default() -> Self {
        Self {
            router: Router::new(),
            routes: Vec::new(),
        }
    }
}

impl<S: Clone + Send + Sync + 'static> Serves<S> {
    /// No routes yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `handler` serving `GET` at `path`, answering under `kinds`.
    #[must_use]
    pub fn get<H, T>(self, path: &'static str, handler: H, kinds: &[Kind]) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.served(Method::GET, path, get(handler), Taking::Nothing, kinds)
    }

    /// `handler` serving `POST` at `path`, taking what `taking` says and answering under
    /// `kinds`.
    #[must_use]
    pub fn post<H, T>(self, path: &'static str, handler: H, taking: Taking, kinds: &[Kind]) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.served(Method::POST, path, post(handler), taking, kinds)
    }

    /// `handler` serving `DELETE` at `path`, taking what `taking` says and answering under
    /// `kinds`.
    #[must_use]
    pub fn delete<H, T>(
        self,
        path: &'static str,
        handler: H,
        taking: Taking,
        kinds: &[Kind],
    ) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.served(Method::DELETE, path, delete(handler), taking, kinds)
    }

    /// These routes and `other`'s.
    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        self.router = self.router.merge(other.router);
        self.routes.extend(other.routes);
        self
    }

    /// The routes, as the contract lists them.
    #[must_use]
    pub fn routes(&self) -> &[Route] {
        &self.routes
    }

    /// The router, with the state every route shares.
    pub fn with_state<S2>(self, state: S) -> Router<S2> {
        self.router.with_state(state)
    }

    /// One route routed and listed.
    fn served(
        mut self,
        method: Method,
        path: &'static str,
        routed: MethodRouter<S>,
        taking: Taking,
        kinds: &[Kind],
    ) -> Self {
        self.router = self.router.route(path, routed);
        let mut kinds: Vec<&'static str> = kinds.iter().map(|kind| kind.as_str()).collect();
        kinds.sort_unstable();
        kinds.dedup();
        self.routes.push(Route {
            method,
            path,
            body: taking == Taking::Body,
            kinds,
        });
        self
    }
}

/// `method` as the token HTTP names it by, which is case-sensitive.
fn spelled<W: Serializer>(method: &Method, writer: W) -> Result<W::Ok, W::Error> {
    writer.serialize_str(method.as_str())
}

#[cfg(test)]
mod tests;
