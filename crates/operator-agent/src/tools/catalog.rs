use operator_runtime::ToolSpec as RuntimeToolSpec;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannerToolSummary {
    pub mode: String,
    pub arguments: Vec<String>,
}

impl From<RuntimeToolSpec> for AgentToolSpec {
    fn from(spec: RuntimeToolSpec) -> Self {
        Self {
            name: spec.name.to_string(),
            description: spec.description.to_string(),
            input_schema: spec.input_schema,
            read_only: !spec.has_side_effects,
        }
    }
}

impl AgentToolSpec {
    pub fn planner_summary(&self) -> PlannerToolSummary {
        let required = self
            .input_schema
            .get("required")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .unwrap_or_default();

        let arguments = self
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .map(|properties| {
                let mut names = properties.keys().cloned().collect::<Vec<_>>();
                names.sort();
                names
                    .into_iter()
                    .map(|name| {
                        let descriptor = properties
                            .get(&name)
                            .map(schema_descriptor)
                            .unwrap_or_else(|| "value".into());
                        if required.contains(name.as_str()) {
                            format!("{name}: {descriptor} (required)")
                        } else {
                            format!("{name}: {descriptor}")
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        PlannerToolSummary {
            mode: if self.read_only {
                "read_only".into()
            } else {
                "side_effect".into()
            },
            arguments,
        }
    }
}

fn schema_descriptor(schema: &Value) -> String {
    if let Some(options) = schema.get("enum").and_then(Value::as_array) {
        let values = options
            .iter()
            .filter_map(|value| match value {
                Value::String(text) => Some(text.clone()),
                Value::Number(number) => Some(number.to_string()),
                Value::Bool(flag) => Some(flag.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !values.is_empty() {
            return format!("enum({})", values.join("|"));
        }
    }

    if let Some(types) = schema.get("type") {
        match types {
            Value::String(kind) => return typed_descriptor(kind, schema),
            Value::Array(items) => {
                let variants = items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|kind| typed_descriptor(kind, schema))
                    .collect::<Vec<_>>();
                if !variants.is_empty() {
                    return variants.join(" | ");
                }
            }
            _ => {}
        }
    }

    "value".into()
}

fn typed_descriptor(kind: &str, schema: &Value) -> String {
    match kind {
        "object" => object_descriptor(schema),
        "array" => array_descriptor(schema),
        "string" | "number" | "integer" | "boolean" | "null" => kind.into(),
        _ => "value".into(),
    }
}

fn object_descriptor(schema: &Value) -> String {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return "object".into();
    };

    let mut names = properties.keys().cloned().collect::<Vec<_>>();
    names.sort();
    let preview = names
        .into_iter()
        .take(3)
        .map(|name| {
            let nested = properties
                .get(&name)
                .map(schema_descriptor)
                .unwrap_or_else(|| "value".into());
            format!("{name}: {nested}")
        })
        .collect::<Vec<_>>()
        .join(", ");

    if preview.is_empty() {
        "object".into()
    } else {
        format!("object {{{preview}}}")
    }
}

fn array_descriptor(schema: &Value) -> String {
    let Some(items) = schema.get("items") else {
        return "array".into();
    };

    format!("array<{}>", schema_descriptor(items))
}
