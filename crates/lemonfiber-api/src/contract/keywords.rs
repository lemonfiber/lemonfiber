//! The JSON Schema keywords the artefact may use: those every SDK generator reads.
//!
//! A generator meeting a keyword it does not read refuses the artefact, so no SDK takes
//! the release. This is the one list the artefact is held to; each generator holds what
//! it reads in its own language and refuses anything beyond it.

/// Every keyword a kind's schema, and each definition it carries, may use.
pub const KINDS: [&str; 27] = [
    "$defs",
    "$ref",
    "$schema",
    "additionalProperties",
    "anyOf",
    "const",
    "default",
    "description",
    "enum",
    "examples",
    "exclusiveMaximum",
    "exclusiveMinimum",
    "format",
    "items",
    "maxItems",
    "maxLength",
    "maximum",
    "minItems",
    "minLength",
    "minimum",
    "oneOf",
    "pattern",
    "properties",
    "required",
    "title",
    "type",
    "uniqueItems",
];

/// The keywords a body's schema may use beyond a kind's, read by the generators that write
/// a request.
pub const BODIES: [&str; 1] = ["prefixItems"];
