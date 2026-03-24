use std::collections::HashMap;

use serde_json::Value;

use crate::{tools::AgentToolSpec, AgentError};

use super::AgentDecision;

#[derive(Clone, Debug, Default)]
pub struct DecisionValidator {
    tools: HashMap<String, AgentToolSpec>,
}

impl DecisionValidator {
    pub fn new(tools: &[AgentToolSpec]) -> Self {
        Self {
            tools: tools
                .iter()
                .cloned()
                .map(|tool| (tool.name.clone(), tool))
                .collect(),
        }
    }

    pub fn validate(&self, decision: &AgentDecision) -> Result<(), AgentError> {
        let AgentDecision::CallTool {
            name, arguments, ..
        } = decision
        else {
            return Ok(());
        };

        let spec = self.tools.get(name).ok_or_else(|| {
            AgentError::Planner(format!(
                "planner tool `{name}` is not available for the current target or runtime policy"
            ))
        })?;

        if !arguments.is_object() {
            return Err(AgentError::Planner(
                "planner tool arguments must be encoded as a JSON object".into(),
            ));
        }

        validate_schema(arguments, &spec.input_schema, &spec.input_schema).map_err(|message| {
            AgentError::Planner(format!(
                "planner tool `{name}` failed schema validation: {message}"
            ))
        })
    }
}

fn validate_schema(value: &Value, schema: &Value, root: &Value) -> Result<(), String> {
    match schema {
        Value::Bool(true) => return Ok(()),
        Value::Bool(false) => return Err("schema forbids this value".into()),
        _ => {}
    }

    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let resolved = resolve_ref(root, reference)?;
        validate_schema(value, resolved, root)?;
    }

    if let Some(options) = schema.get("allOf").and_then(Value::as_array) {
        for option in options {
            validate_schema(value, option, root)?;
        }
    }

    if let Some(options) = schema.get("anyOf").and_then(Value::as_array) {
        let matched = options
            .iter()
            .any(|option| validate_schema(value, option, root).is_ok());
        if !matched {
            return Err("value did not match any allowed schema branch".into());
        }
    }

    if let Some(options) = schema.get("oneOf").and_then(Value::as_array) {
        let matches = options
            .iter()
            .filter(|option| validate_schema(value, option, root).is_ok())
            .count();
        if matches != 1 {
            return Err(format!(
                "value matched {matches} schema branches but exactly one is required"
            ));
        }
    }

    if let Some(expected) = schema.get("const") {
        if value != expected {
            return Err(format!("value must equal {expected}"));
        }
    }

    if let Some(options) = schema.get("enum").and_then(Value::as_array) {
        if !options.iter().any(|option| option == value) {
            return Err("value is not one of the allowed enum members".into());
        }
    }

    if let Some(type_schema) = schema.get("type") {
        validate_type(value, type_schema)?;
    }

    if let Some(object) = value.as_object() {
        validate_object(object, schema, root)?;
    }

    if let Some(items) = value.as_array() {
        validate_array(items, schema, root)?;
    }

    Ok(())
}

fn validate_type(value: &Value, type_schema: &Value) -> Result<(), String> {
    match type_schema {
        Value::String(expected) => {
            if matches_type(value, expected) {
                Ok(())
            } else {
                Err(format!("value does not match schema type `{expected}`"))
            }
        }
        Value::Array(options) => {
            let mut allowed = Vec::new();
            for option in options {
                let expected = option
                    .as_str()
                    .ok_or_else(|| "schema type array must contain only strings".to_string())?;
                allowed.push(expected);
                if matches_type(value, expected) {
                    return Ok(());
                }
            }

            Err(format!(
                "value does not match any allowed schema type: {}",
                allowed.join(", ")
            ))
        }
        _ => Err("schema `type` must be a string or array".into()),
    }
}

fn matches_type(value: &Value, expected: &str) -> bool {
    match expected {
        "null" => value.is_null(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "string" => value.is_string(),
        _ => true,
    }
}

fn validate_object(
    object: &serde_json::Map<String, Value>,
    schema: &Value,
    root: &Value,
) -> Result<(), String> {
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for key in required {
            let key = key
                .as_str()
                .ok_or_else(|| "schema `required` entries must be strings".to_string())?;
            if !object.contains_key(key) {
                return Err(format!("missing required property `{key}`"));
            }
        }
    }

    let properties = schema.get("properties").and_then(Value::as_object);
    if let Some(properties) = properties {
        for (key, subschema) in properties {
            if let Some(value) = object.get(key) {
                validate_schema(value, subschema, root)
                    .map_err(|message| format!("property `{key}` {message}"))?;
            }
        }
    }

    match schema.get("additionalProperties") {
        Some(Value::Bool(false)) => {
            for key in object.keys() {
                let known = properties.is_some_and(|properties| properties.contains_key(key));
                if !known {
                    return Err(format!("property `{key}` is not allowed by schema"));
                }
            }
        }
        Some(additional_schema @ Value::Object(_)) => {
            for (key, value) in object {
                let known = properties.is_some_and(|properties| properties.contains_key(key));
                if !known {
                    validate_schema(value, additional_schema, root)
                        .map_err(|message| format!("property `{key}` {message}"))?;
                }
            }
        }
        _ => {}
    }

    Ok(())
}

fn validate_array(items: &[Value], schema: &Value, root: &Value) -> Result<(), String> {
    if let Some(min_items) = schema.get("minItems").and_then(Value::as_u64) {
        if items.len() < min_items as usize {
            return Err(format!("array must contain at least {min_items} items"));
        }
    }

    if let Some(max_items) = schema.get("maxItems").and_then(Value::as_u64) {
        if items.len() > max_items as usize {
            return Err(format!("array must contain at most {max_items} items"));
        }
    }

    match schema.get("items") {
        Some(item_schema @ Value::Object(_)) | Some(item_schema @ Value::Bool(_)) => {
            for (index, item) in items.iter().enumerate() {
                validate_schema(item, item_schema, root)
                    .map_err(|message| format!("item {index} {message}"))?;
            }
        }
        Some(Value::Array(item_schemas)) => {
            for (index, item) in items.iter().enumerate() {
                let Some(item_schema) = item_schemas.get(index) else {
                    break;
                };
                validate_schema(item, item_schema, root)
                    .map_err(|message| format!("item {index} {message}"))?;
            }
        }
        _ => {}
    }

    Ok(())
}

fn resolve_ref<'a>(root: &'a Value, reference: &str) -> Result<&'a Value, String> {
    let pointer = reference
        .strip_prefix('#')
        .ok_or_else(|| format!("only local schema refs are supported: {reference}"))?;
    root.pointer(pointer)
        .ok_or_else(|| format!("missing schema reference `{reference}`"))
}
