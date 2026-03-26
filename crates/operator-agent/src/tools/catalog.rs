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
        let required = collect_required(&self.input_schema, &self.input_schema);
        let properties = collect_properties(&self.input_schema, &self.input_schema);
        let mut names = properties.keys().cloned().collect::<Vec<_>>();
        names.sort();
        let arguments = names
            .into_iter()
            .map(|name| {
                let descriptor = properties
                    .get(&name)
                    .map(|schema| schema_descriptor(schema, &self.input_schema))
                    .unwrap_or_else(|| "value".into());
                if required.contains(name.as_str()) {
                    format!("{name}: {descriptor} (required)")
                } else {
                    format!("{name}: {descriptor}")
                }
            })
            .collect();

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

fn schema_descriptor(schema: &Value, root: &Value) -> String {
    let schema = resolve_schema(schema, root);

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

    if let Some(descriptor) = composite_descriptor(schema, root, "anyOf") {
        return descriptor;
    }

    if let Some(descriptor) = composite_descriptor(schema, root, "oneOf") {
        return descriptor;
    }

    if let Some(descriptor) = composite_descriptor(schema, root, "allOf") {
        return descriptor;
    }

    if let Some(types) = schema.get("type") {
        match types {
            Value::String(kind) => return typed_descriptor(kind, schema, root),
            Value::Array(items) => {
                let variants = items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|kind| typed_descriptor(kind, schema, root))
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

fn typed_descriptor(kind: &str, schema: &Value, root: &Value) -> String {
    match kind {
        "object" => object_descriptor(schema, root),
        "array" => array_descriptor(schema, root),
        "string" | "number" | "integer" | "boolean" | "null" => kind.into(),
        _ => "value".into(),
    }
}

fn object_descriptor(schema: &Value, root: &Value) -> String {
    let properties = collect_properties(schema, root);
    if properties.is_empty() {
        return "object".into();
    }

    let mut names = properties.keys().cloned().collect::<Vec<_>>();
    names.sort();
    let preview = names
        .into_iter()
        .take(3)
        .map(|name| {
            let nested = properties
                .get(&name)
                .map(|schema| schema_descriptor(schema, root))
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

fn array_descriptor(schema: &Value, root: &Value) -> String {
    let Some(items) = schema.get("items") else {
        return "array".into();
    };

    format!("array<{}>", schema_descriptor(items, root))
}

fn composite_descriptor(schema: &Value, root: &Value, key: &str) -> Option<String> {
    let options = schema.get(key).and_then(Value::as_array)?;
    let mut variants = options
        .iter()
        .map(|option| schema_descriptor(option, root))
        .filter(|descriptor| descriptor != "value")
        .collect::<Vec<_>>();
    variants.sort();
    variants.dedup();
    if variants.is_empty() {
        None
    } else {
        Some(variants.join(" | "))
    }
}

fn resolve_schema<'a>(schema: &'a Value, root: &'a Value) -> &'a Value {
    match schema.get("$ref").and_then(Value::as_str) {
        Some(reference) => resolve_ref(root, reference).unwrap_or(schema),
        None => schema,
    }
}

fn resolve_ref<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    let pointer = reference.strip_prefix('#')?;
    root.pointer(pointer)
}

fn collect_required<'a>(schema: &'a Value, root: &'a Value) -> std::collections::BTreeSet<&'a str> {
    let schema = resolve_schema(schema, root);
    let mut required = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    if let Some(items) = schema.get("allOf").and_then(Value::as_array) {
        for item in items {
            required.extend(collect_required(item, root));
        }
    }
    required
}

fn collect_properties(schema: &Value, root: &Value) -> std::collections::BTreeMap<String, Value> {
    let schema = resolve_schema(schema, root);
    let mut properties = std::collections::BTreeMap::new();

    if let Some(items) = schema.get("allOf").and_then(Value::as_array) {
        for item in items {
            properties.extend(collect_properties(item, root));
        }
    }

    if let Some(local) = schema.get("properties").and_then(Value::as_object) {
        for (key, value) in local {
            properties.insert(key.clone(), value.clone());
        }
    }

    properties
}
