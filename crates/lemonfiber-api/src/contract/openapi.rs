//! The contract artefact rendered as one self-contained OpenAPI 3.1 document.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use super::layout::Files;
use super::path::INDEX;

/// Where the OpenAPI document is kept, relative to the workspace root.
pub const OPENAPI: &str = "contract/web-api.openapi.json";

/// The header every request carries this run's token in.
const TOKEN_HEADER: &str = "X-Lemonfiber-Token";

/// Where a reference inside the document points.
const COMPONENTS: &str = "#/components/schemas/";

/// The name the union of every kind is kept under among the components.
const ANY_KIND: &str = "envelope";

/// The OpenAPI document of the web API, rendered from the artefact's files.
///
/// # Errors
///
/// Every file the index names that is missing or is not JSON, since a document
/// rendered without it would describe less than the artefact does.
pub fn rendered(files: &Files) -> Result<Value, Vec<String>> {
    let mut faults = Vec::new();
    let mut read = |path: &str| -> Value {
        match files.get(path).map(|text| serde_json::from_str(text)) {
            Some(Ok(value)) => value,
            _ => {
                faults.push(format!(
                    "{path} is named by the artefact and cannot be read"
                ));
                Value::Null
            }
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
    for (route, path) in named("bodies") {
        let mut schema = read(&path);
        undeclared(&mut schema);
        let name = format!("body-{}", stem(&path));
        schemas.insert(name.clone(), inlined(schema));
        paths.insert(
            route,
            json!({ "post": {
                "requestBody": { "required": true, "content": { "application/json": { "schema": reference(&name) } } },
                "responses": answered(&[ANY_KIND.to_owned()]),
            } }),
        );
    }
    for (action, path) in named("actions") {
        let listed = read(&path);
        paths.insert(
            format!("/api/actions/{action}"),
            json!({ "post": acted(&listed) }),
        );
    }
    let reads = index
        .get("reads")
        .and_then(Value::as_str)
        .map_or(Value::Null, &mut read);
    for listed in reads.as_array().into_iter().flatten() {
        if let Some(route) = listed.get("path").and_then(Value::as_str) {
            paths.insert(route.to_owned(), json!({ "get": asked(route, listed) }));
        }
    }
    paths.insert("/api/jobs/{job}".to_owned(), redeemed());
    paths.insert("/api/events".to_owned(), streamed(index.get("events")));

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
    let mut responses = answered(&[ANY_KIND.to_owned()]);
    responses["202"] = json!({
        "description": "Accepted, and named so that what became of it can be asked.",
        "content": { "application/json": { "schema": reference(&kind_name("job")) } },
    });
    json!({
        "requestBody": {
            "content": { "application/json": { "schema": { "type": "object", "properties": properties } } },
        },
        "responses": responses,
    })
}

/// One read, as the operation that asks it.
fn asked(route: &str, listed: &Value) -> Value {
    let mut parameters: Vec<Value> = route
        .split('/')
        .filter_map(|segment| segment.strip_prefix('{')?.strip_suffix('}'))
        .map(|name| json!({ "name": name, "in": "path", "required": true, "schema": { "type": "string" } }))
        .collect();
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

/// What a name for work is redeemed and released with.
fn redeemed() -> Value {
    let job =
        json!([{ "name": "job", "in": "path", "required": true, "schema": { "type": "string" } }]);
    let mut responses = answered(&[ANY_KIND.to_owned()]);
    responses["202"] = json!({
        "description": "Still going.",
        "content": { "application/json": { "schema": reference(&kind_name("job")) } },
    });
    json!({
        "parameters": job,
        "get": { "responses": responses.clone() },
        "delete": { "responses": responses },
    })
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
