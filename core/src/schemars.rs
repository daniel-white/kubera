use schemars::{json_schema, Schema, SchemaGenerator};

pub fn cidr_array(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "array",
        "items": {
            "type": "string",
            "format": "cidr"
        },
        "minItems": 1,
        "uniqueItems": true
    })
}

pub fn http_header_name(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "string",
        "pattern": "^[a-zA-Z0-9!#$%&'*+.^_`|~-]+:\\s?.*$"
    })
}

pub fn http_header_name_set(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "array",
        "items": {
            "type": "string",
            "pattern": "^[a-zA-Z0-9!#$%&'*+.^_`|~-]+:\\s?.*$"
        },
        "uniqueItems": true
    })
}

pub fn http_header_map(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "object",
        "patternProperties": {
            "^[a-zA-Z0-9!#$%&'*+.^_`|~-]+:\\s?.*$": {
                "type": "array",
                "items": {
                    "type": "string"
                },
                "minItems": 1,
            }
        }
    })
}
