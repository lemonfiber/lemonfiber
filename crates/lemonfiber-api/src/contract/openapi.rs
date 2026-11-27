//! The contract artefact rendered as one self-contained `OpenAPI` 3.1 document.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use super::layout::Files;
use super::path::INDEX;
use crate::actions::ACTION;
use crate::events::PATH as STREAM;

/// Where the `OpenAPI` document is kept, relative to the workspace root.
pub const OPENAPI: &str = "contract/web-api.openapi.json";

/// The header every request carries this run's token in.
const TOKEN_HEADER: &str = "X-Lemonfiber-Token";

/// Where a reference inside the document points.
const COMPONENTS: &str = "#/components/schemas/";

/// The name the union of every kind is kept under among the components.
const ANY_KIND: &str = "envelope";

/// The `OpenAPI` document of the web API, rendered from the artefact's files.
///
/// # Errors
///
/// Every file the index names that is missing or is not JSON, since a document
/// rendered without it would describe less than the artefact does.
pub fn rendered(files: &Files) -> Result<Value, Vec<String>> {
    let mut faults = Vec::new();
    let mut read = |path: &str| -> Value {
        if let Some(Ok(value)) = files.get(path).map(|text| serde_json::from_str(text)) {
            value
        } else {
            faults.push(format!(
                "{path} is named by the artefact and cannot be read"
            ));
            Value::Null
        }
    };
    let index = read(INDEX);
    let named = |key: &str| -> BTreeMap<String, String> {
        index
            .get(key)
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(name, path)| Some((name.clone(), path.as_str()?.to_owned())))
            .collect()
    };

    let mut schemas = Map::new();
    let mut dialect = Value::Null;
    for path in files.keys().filter(|path| path.starts_with("defs/")) {
        let name = stem(path);
        let mut schema = read(path);
        dialect = undeclared(&mut schema).unwrap_or(dialect);
        schemas.insert(name.to_owned(), inlined(schema));
    }
    let kinds = named("kinds");
    for (kind, path) in &kinds {
        let mut schema = read(path);
        dialect = undeclared(&mut schema).unwrap_or(dialect);
        schemas.insert(kind_name(kind), inlined(schema));
    }
    schemas.insert(
        ANY_KIND.to_owned(),
        json!({ "oneOf": kinds.keys().map(|kind| reference(&kind_name(kind))).collect::<Vec<_>>() }),
    );
    let mut paths = Map::new();
    let mut bodies = BTreeMap::new();
    for (route, path) in named("bodies") {
        let mut schema = read(&path);
        undeclared(&mut schema);
        let name = format!("body-{}", stem(&path));
        schemas.insert(name.clone(), inlined(schema));
        bodies.insert(route, name);
    }
    for (action, path) in named("actions") {
        let listed = read(&path);
        paths.insert(
            format!("/api/actions/{action}"),
            json!({ "post": acted(&listed) }),
        );
    }
    let mut listed_in = |key: &str| {
        index
            .get(key)
            .and_then(Value::as_str)
            .map_or(Value::Null, &mut read)
    };
    let reads = listed_in("reads");
    let routes = listed_in("routes");
    for listed in reads.as_array().into_iter().flatten() {
        if let Some(route) = listed.get("path").and_then(Value::as_str) {
            paths.insert(route.to_owned(), json!({ "get": asked(route, listed) }));
        }
    }
    paths.insert(STREAM.to_owned(), streamed(index.get("events")));
    routed(&mut paths, &routes, &bodies);

    if !faults.is_empty() {
        return Err(faults);
    }
    Ok(json!({
        "openapi": "3.1.0",
        "info": {
            "title": "lemonfiber web API",
            "version": index.get("api_version").cloned().unwrap_or(Value::Null),
        },
        "jsonSchemaDialect": dialect,
        "security": [{ "token": [] }],
        "paths": paths,
        "components": {
            "schemas": schemas,
            "securitySchemes": {
                "token": { "type": "apiKey", "in": "header", "name": TOKEN_HEADER },
            },
        },
    }))
}

/// A file's name without its directory or its `.json`.
fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".json").unwrap_or(name)
}

/// The component a kind's envelope is kept under.
fn kind_name(kind: &str) -> String {
    format!("kind-{kind}")
}

/// A reference to one component.
fn reference(name: &str) -> Value {
    json!({ "$ref": format!("{COMPONENTS}{name}") })
}

/// `schema` without the dialect it names, which the document names once, and that dialect.
fn undeclared(schema: &mut Value) -> Option<Value> {
    schema.as_object_mut()?.remove("$schema")
}

/// `schema` with every reference to a definition's file pointed at that definition's component.
fn inlined(mut schema: Value) -> Value {
    pointed(&mut schema);
    schema
}

/// Every reference under `node` pointed at the component its file is kept as.
fn pointed(node: &mut Value) {
    match node {
        Value::Object(fields) => {
            if let Some(Value::String(target)) = fields.get_mut("$ref") {
                *target = format!("{COMPONENTS}{}", stem(target));
            }
            fields.values_mut().for_each(pointed);
        }
        Value::Array(items) => items.iter_mut().for_each(pointed),
        _ => {}
    }
}

/// What a request may be answered with: one of `kinds` on success, an error otherwise.
fn answered(kinds: &[String]) -> Value {
    let schema = match kinds {
        [one] => reference(one),
        many => json!({ "oneOf": many.iter().map(|kind| reference(kind)).collect::<Vec<_>>() }),
    };
    json!({
        "200": { "description": "Answered.", "content": { "application/json": { "schema": schema } } },
        "default": refused(),
    })
}

/// One of `kinds` answered, and work still going answered `202` with its name, described
/// as `going`.
fn accepted(kinds: &[String], going: &str) -> Value {
    let mut responses = answered(kinds);
    if let Some(each) = responses.as_object_mut() {
        each.insert(
            "202".to_owned(),
            json!({
                "description": going,
                "content": { "application/json": { "schema": reference(&kind_name("job")) } },
            }),
        );
    }
    responses
}

/// The error envelope every refusal is answered with.
fn refused() -> Value {
    json!({
        "description": "Refused.",
        "content": { "application/json": { "schema": reference(&kind_name("error")) } },
    })
}

/// One action, as the operation that takes it.
fn acted(listed: &Value) -> Value {
    let properties: Map<String, Value> = listed
        .get("arguments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|argument| {
            let name = argument.get("name")?.as_str()?;
            Some((name.to_owned(), inlined(argument.get("type")?.clone())))
        })
        .collect();
    let responses = accepted(
        &[ANY_KIND.to_owned()],
        "Accepted, and named so that what became of it can be asked.",
    );
    json!({
        "requestBody": {
            "content": { "application/json": { "schema": { "type": "object", "properties": properties } } },
        },
        "responses": responses,
    })
}

/// Each parameter `route` names in its path, as the operation declares it.
fn in_path(route: &str) -> Vec<Value> {
    route
        .split('/')
        .filter_map(|segment| segment.strip_prefix('{')?.strip_suffix('}'))
        .map(|name| json!({ "name": name, "in": "path", "required": true, "schema": { "type": "string" } }))
        .collect()
}

/// One read, as the operation that asks it.
fn asked(route: &str, listed: &Value) -> Value {
    let mut parameters = in_path(route);
    parameters.extend(
        listed
            .get("parameters")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|parameter| {
                let one = json!({ "type": "string" });
                let schema = if parameter.get("repeatable") == Some(&Value::Bool(true)) {
                    json!({ "type": "array", "items": one })
                } else {
                    one
                };
                json!({ "name": parameter.get("name").cloned().unwrap_or(Value::Null), "in": "query", "schema": schema })
            }),
    );
    let responses = if listed.get("file") == Some(&Value::Bool(true)) {
        json!({
            "200": { "description": "The file.", "content": { "application/octet-stream": { "schema": { "type": "string", "contentMediaType": "application/octet-stream" } } } },
            "default": refused(),
        })
    } else {
        let kinds: Vec<String> = listed
            .get("kinds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(kind_name)
            .collect();
        answered(&kinds)
    };
    json!({ "parameters": parameters, "responses": responses })
}

/// Every route `routes` lists that no read, action or stream already describes, added to
/// `paths` as the operation serving it.
fn routed(paths: &mut Map<String, Value>, routes: &Value, bodies: &BTreeMap<String, String>) {
    for listed in routes.as_array().into_iter().flatten() {
        let (Some(method), Some(route)) = (
            listed.get("method").and_then(Value::as_str),
            listed.get("path").and_then(Value::as_str),
        ) else {
            continue;
        };
        let method = method.to_ascii_lowercase();
        let described = paths
            .get(route)
            .is_some_and(|path| path.get(&method).is_some());
        if route == ACTION || described {
            continue;
        }
        let operation = served(route, &method, listed, bodies.get(route));
        if let Some(path) = paths
            .entry(route)
            .or_insert_with(|| json!({}))
            .as_object_mut()
        {
            path.insert(method, operation);
        }
    }
}

/// One route neither a read, an action nor the stream, as the operation serving it: its
/// path's parameters, the body it takes where it takes one, and the kinds it answers with,
/// work still going answered `202` with its name where it may be.
fn served(route: &str, method: &str, listed: &Value, body: Option<&String>) -> Value {
    let kinds: Vec<String> = listed
        .get("kinds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(kind_name)
        .collect();
    let responses = if kinds.contains(&kind_name("job")) {
        accepted(&kinds, "Still going.")
    } else {
        answered(&kinds)
    };
    let mut operation = Map::new();
    operation.insert("parameters".to_owned(), Value::Array(in_path(route)));
    operation.insert("responses".to_owned(), responses);
    if let Some(body) = body {
        let taken = json!({
            "required": method != "delete",
            "content": { "application/json": { "schema": reference(body) } },
        });
        operation.insert("requestBody".to_owned(), taken);
    }
    Value::Object(operation)
}

/// The event stream, with its heartbeat and each kind of event it says.
fn streamed(events: Option<&Value>) -> Value {
    let mut described = events.cloned().unwrap_or(Value::Null);
    for (kind, said) in described
        .get_mut("kinds")
        .and_then(Value::as_object_mut)
        .into_iter()
        .flatten()
    {
        said["schema"] = reference(&kind_name(kind));
    }
    json!({ "get": {
        "responses": {
            "200": {
                "description": "Server-sent events, each named for the kind of the envelope its data carries.",
                "content": { "text/event-stream": { "schema": { "type": "string" } } },
            },
            "default": refused(),
        },
        "x-lemonfiber-events": described,
    } })
}
