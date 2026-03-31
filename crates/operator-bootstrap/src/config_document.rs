use std::{
    fs,
    path::{Path, PathBuf},
};

use operator_core::{DriverConfig, OperatorError, TargetId};
use operator_runtime::{NamedTargetConfig, RuntimeConfig};
use serde::Serialize;
use serde_json::Value as JsonValue;
use toml::Value as TomlValue;
use toml_edit::{value as edit_value, DocumentMut, Item, Table};

use crate::{
    parse_bootstrap_config, parse_runtime_config, validate_supported_model_selector,
    AgentModelProviderConfig, BootstrapConfig,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetConfigFieldPath {
    Platform,
    Driver,
    Description,
    DriverConfig(Vec<String>),
}

impl TargetConfigFieldPath {
    pub fn parse_set(path: &str) -> Result<Self, OperatorError> {
        parse_target_path(path, true)
    }

    pub fn parse_unset(path: &str) -> Result<Self, OperatorError> {
        if matches!(path, "platform" | "driver") {
            return Err(OperatorError::Platform(format!(
                "target path `{path}` cannot be removed"
            )));
        }
        let parsed = parse_target_path(path, false)?;
        match parsed {
            Self::Description | Self::DriverConfig(_) => Ok(parsed),
            Self::Platform | Self::Driver => Err(OperatorError::Platform(format!(
                "target path `{path}` cannot be removed"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelConfigFieldPath {
    ApiKey,
    BaseUrl,
    ModelName,
}

impl ModelConfigFieldPath {
    pub fn parse_set(path: &str) -> Result<Self, OperatorError> {
        parse_model_path(path)
    }

    pub fn parse_unset(path: &str) -> Result<Self, OperatorError> {
        parse_model_path(path)
    }
}

pub fn parse_target_set_expression(
    expression: &str,
) -> Result<(TargetConfigFieldPath, TomlValue), OperatorError> {
    let (path, raw_value) = expression.split_once('=').ok_or_else(|| {
        OperatorError::Platform(format!(
            "target set expression `{expression}` must use <path>=<value> syntax"
        ))
    })?;
    let path = TargetConfigFieldPath::parse_set(path)?;
    let value = parse_toml_value(raw_value)?;
    Ok((path, value))
}

pub fn parse_model_set_expression(
    expression: &str,
) -> Result<(ModelConfigFieldPath, TomlValue), OperatorError> {
    let (path, raw_value) = expression.split_once('=').ok_or_else(|| {
        OperatorError::Platform(format!(
            "model set expression `{expression}` must use <field>=<value> syntax"
        ))
    })?;
    let path = ModelConfigFieldPath::parse_set(path)?;
    let value = parse_toml_value(raw_value)?;
    Ok((path, value))
}

#[derive(Debug, Clone)]
pub struct RuntimeConfigDocument {
    path: PathBuf,
    document: DocumentMut,
}

impl RuntimeConfigDocument {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, OperatorError> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Ok(Self {
                path,
                document: DocumentMut::new(),
            });
        }

        let contents = fs::read_to_string(&path)?;
        let document = contents.parse::<DocumentMut>().map_err(|error| {
            OperatorError::Platform(format!(
                "invalid runtime config at {}: {error}",
                path.display()
            ))
        })?;

        // Keep read-only bootstrap semantics aligned with the editable document path.
        parse_bootstrap_config(&contents, &path)?;

        Ok(Self { path, document })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn to_runtime_config(&self) -> Result<RuntimeConfig, OperatorError> {
        parse_runtime_config(&self.document.to_string(), &self.path)
    }

    pub fn to_bootstrap_config(&self) -> Result<BootstrapConfig, OperatorError> {
        parse_bootstrap_config(&self.document.to_string(), &self.path)
    }

    pub fn set_default_target(&mut self, target: &TargetId) {
        let runtime = ensure_child_table(self.document.as_item_mut(), "runtime");
        runtime["default_target"] = edit_value(target.to_string());
    }

    pub fn set_default_model_selector(&mut self, selector: &str) -> Result<(), OperatorError> {
        validate_supported_model_selector(selector)?;
        let model = self.agent_model_table_mut();
        model["default"] = edit_value(selector);
        Ok(())
    }

    pub fn set_model_provider(
        &mut self,
        name: &str,
        provider: &AgentModelProviderConfig,
    ) -> Result<(), OperatorError> {
        let table = self.model_provider_table_mut(name)?;
        set_optional_string_field(table, "api_key", provider.api_key.as_deref());
        set_optional_string_field(table, "base_url", provider.base_url.as_deref());
        set_optional_string_field(table, "model_name", provider.model_name.as_deref());
        self.prune_agent_model_tables();
        Ok(())
    }

    pub fn set_model_provider_value(
        &mut self,
        name: &str,
        path: &ModelConfigFieldPath,
        new_value: TomlValue,
    ) -> Result<(), OperatorError> {
        let rendered_path = render_model_path(path);
        let string = new_value.as_str().ok_or_else(|| {
            OperatorError::Platform(format!(
                "model field `{rendered_path}` only accepts string values"
            ))
        })?;
        let table = self.model_provider_table_mut(name)?;
        table[rendered_path.as_str()] = edit_value(string);
        Ok(())
    }

    pub fn unset_model_provider_value(
        &mut self,
        name: &str,
        path: &ModelConfigFieldPath,
    ) -> Result<(), OperatorError> {
        validate_supported_model_selector(name)?;

        let Some(agent_table) = self.document.as_table_mut().get_mut("agent") else {
            return Ok(());
        };
        let Some(model_table) = agent_table
            .as_table_mut()
            .and_then(|table| table.get_mut("model"))
        else {
            return Ok(());
        };
        let Some(provider_table) = model_table
            .as_table_mut()
            .and_then(|table| table.get_mut("provider"))
        else {
            return Ok(());
        };
        let Some(provider_entry) = provider_table
            .as_table_mut()
            .and_then(|table| table.get_mut(name))
        else {
            return Ok(());
        };
        if let Some(table) = provider_entry.as_table_mut() {
            table.remove(render_model_path(path).as_str());
        }
        self.prune_agent_model_tables();
        Ok(())
    }

    pub fn set_named_target(
        &mut self,
        name: &str,
        target: &NamedTargetConfig,
    ) -> Result<(), OperatorError> {
        let target_item = self.named_target_item_mut(name);
        *target_item = Item::Table(Table::new());
        let target_table = target_item.as_table_mut().expect("target table");

        target_table["platform"] = edit_value(&target.platform);
        target_table["driver"] = edit_value(&target.driver);
        match target.description.as_deref() {
            Some(description) => target_table["description"] = edit_value(description),
            None => {
                target_table.remove("description");
            }
        }
        if target.driver_config.is_empty() {
            target_table.remove("driver_config");
        } else {
            target_table["driver_config"] = json_driver_config_to_item(&target.driver_config)?;
        }

        Ok(())
    }

    pub fn remove_named_target(&mut self, name: &str) {
        let targets = ensure_child_table(self.document.as_item_mut(), "targets");
        targets.remove(name);
    }

    pub fn set_target_value(
        &mut self,
        name: &str,
        path: &TargetConfigFieldPath,
        new_value: TomlValue,
    ) -> Result<(), OperatorError> {
        match path {
            TargetConfigFieldPath::Platform => {
                let platform = new_value.as_str().ok_or_else(|| {
                    OperatorError::Platform(
                        "target field `platform` only accepts string values".into(),
                    )
                })?;
                self.named_target_table_mut(name)["platform"] = edit_value(platform);
            }
            TargetConfigFieldPath::Driver => {
                let driver = new_value.as_str().ok_or_else(|| {
                    OperatorError::Platform(
                        "target field `driver` only accepts string values".into(),
                    )
                })?;
                self.named_target_table_mut(name)["driver"] = edit_value(driver);
            }
            TargetConfigFieldPath::Description => {
                let description = new_value.as_str().ok_or_else(|| {
                    OperatorError::Platform(
                        "target field `description` only accepts string values".into(),
                    )
                })?;
                self.named_target_table_mut(name)["description"] = edit_value(description);
            }
            TargetConfigFieldPath::DriverConfig(segments) => {
                let mut current = &mut self.named_target_table_mut(name)["driver_config"];
                if current.is_none() || !current.is_table() {
                    *current = Item::Table(Table::new());
                }

                for (index, segment) in segments[..segments.len() - 1].iter().enumerate() {
                    let table = current.as_table_mut().expect("driver_config table");
                    if !table.contains_key(segment) {
                        table[segment] = Item::Table(Table::new());
                    } else if !table[segment].is_table() {
                        return Err(OperatorError::Platform(format!(
                            "target path `{}` conflicts with existing non-table value at `driver_config.{}`",
                            render_target_path(path),
                            segments[..=index].join(".")
                        )));
                    }
                    current = &mut table[segment];
                }

                let table = current.as_table_mut().expect("nested driver_config table");
                table[&segments[segments.len() - 1]] = toml_value_to_item(&new_value)?;
            }
        }

        Ok(())
    }

    pub fn unset_target_value(
        &mut self,
        name: &str,
        path: &TargetConfigFieldPath,
    ) -> Result<(), OperatorError> {
        match path {
            TargetConfigFieldPath::Description => {
                self.named_target_table_mut(name).remove("description");
            }
            TargetConfigFieldPath::DriverConfig(segments) => {
                remove_nested_key(
                    &mut self.named_target_table_mut(name)["driver_config"],
                    segments,
                );
                prune_empty_tables(&mut self.named_target_table_mut(name)["driver_config"]);
                if self.named_target_table_mut(name)["driver_config"]
                    .as_table()
                    .is_some_and(Table::is_empty)
                {
                    self.named_target_table_mut(name).remove("driver_config");
                }
            }
            TargetConfigFieldPath::Platform | TargetConfigFieldPath::Driver => {
                return Err(OperatorError::Platform(format!(
                    "target path `{}` cannot be removed",
                    render_target_path(path)
                )));
            }
        }

        Ok(())
    }

    pub fn save(&self) -> Result<(), OperatorError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, self.document.to_string())?;
        Ok(())
    }

    pub fn render(&self) -> String {
        self.document.to_string()
    }

    pub fn persisted_target_names(&self) -> Vec<String> {
        let mut names = self
            .document
            .as_table()
            .get("targets")
            .and_then(Item::as_table)
            .map(|targets| {
                targets
                    .iter()
                    .filter(|(_, item)| item.is_table())
                    .map(|(name, _)| name.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    pub fn has_persisted_named_target(&self, name: &str) -> bool {
        self.document
            .as_table()
            .get("targets")
            .and_then(Item::as_table)
            .is_some_and(|targets| targets.get(name).is_some_and(Item::is_table))
    }

    fn named_target_item_mut(&mut self, name: &str) -> &mut Item {
        let targets = ensure_child_table(self.document.as_item_mut(), "targets");
        &mut targets[name]
    }

    fn named_target_table_mut(&mut self, name: &str) -> &mut Table {
        let target_item = self.named_target_item_mut(name);
        if !target_item.is_table() {
            *target_item = Item::Table(Table::new());
        }
        target_item.as_table_mut().expect("target table")
    }

    fn agent_model_table_mut(&mut self) -> &mut Table {
        let agent = ensure_child_table(self.document.as_item_mut(), "agent");
        if !agent.contains_key("model") || !agent["model"].is_table() {
            agent["model"] = Item::Table(Table::new());
        }
        agent["model"].as_table_mut().expect("agent.model table")
    }

    fn model_provider_table_mut(&mut self, name: &str) -> Result<&mut Table, OperatorError> {
        validate_supported_model_selector(name)?;
        let model = self.agent_model_table_mut();
        if !model.contains_key("provider") || !model["provider"].is_table() {
            model["provider"] = Item::Table(Table::new());
        }
        let provider = model["provider"].as_table_mut().expect("provider table");
        if !provider.contains_key(name) || !provider[name].is_table() {
            provider[name] = Item::Table(Table::new());
        }
        Ok(provider[name].as_table_mut().expect("provider entry"))
    }

    fn prune_agent_model_tables(&mut self) {
        let document = self.document.as_table_mut();
        let Some(agent_item) = document.get_mut("agent") else {
            return;
        };
        let Some(agent_table) = agent_item.as_table_mut() else {
            return;
        };
        if let Some(model_item) = agent_table.get_mut("model") {
            prune_empty_tables(model_item);
            if model_item.as_table().is_some_and(Table::is_empty) {
                agent_table.remove("model");
            }
        }
        if agent_table.is_empty() {
            document.remove("agent");
        }
    }
}

fn parse_target_path(
    path: &str,
    allow_required_fields: bool,
) -> Result<TargetConfigFieldPath, OperatorError> {
    if path.is_empty() {
        return Err(OperatorError::Platform(
            "target path cannot be empty".into(),
        ));
    }
    if path.starts_with('.') || path.ends_with('.') || path.contains("..") {
        return Err(OperatorError::Platform(format!(
            "target path `{path}` contains an empty segment"
        )));
    }
    if path.contains('[') || path.contains(']') {
        return Err(OperatorError::Platform(format!(
            "target path `{path}` must not use array index syntax"
        )));
    }

    let segments = path.split('.').collect::<Vec<_>>();
    if segments.iter().any(|segment| segment.is_empty()) {
        return Err(OperatorError::Platform(format!(
            "target path `{path}` contains an empty segment"
        )));
    }

    match segments.as_slice() {
        ["platform"] if allow_required_fields => Ok(TargetConfigFieldPath::Platform),
        ["driver"] if allow_required_fields => Ok(TargetConfigFieldPath::Driver),
        ["description"] => Ok(TargetConfigFieldPath::Description),
        ["driver_config"] => Err(OperatorError::Platform(
            "target path `driver_config` must address a concrete driver_config key".into(),
        )),
        ["driver_config", rest @ ..] if !rest.is_empty() => {
            Ok(TargetConfigFieldPath::DriverConfig(
                rest.iter().map(|segment| (*segment).to_string()).collect(),
            ))
        }
        ["targets", ..] | ["runtime", ..] => Err(OperatorError::Platform(format!(
            "target path `{path}` must be relative to a single [targets.<name>] entry"
        ))),
        _ => Err(OperatorError::Platform(format!(
            "target path `{path}` is not part of the standardized target contract"
        ))),
    }
}

fn parse_model_path(path: &str) -> Result<ModelConfigFieldPath, OperatorError> {
    if path.is_empty() {
        return Err(OperatorError::Platform(
            "model field path cannot be empty".into(),
        ));
    }
    if path.starts_with('.') || path.ends_with('.') || path.contains("..") {
        return Err(OperatorError::Platform(format!(
            "model field path `{path}` contains an empty segment"
        )));
    }
    if path.contains('[') || path.contains(']') {
        return Err(OperatorError::Platform(format!(
            "model field path `{path}` must not use array index syntax"
        )));
    }

    let segments = path.split('.').collect::<Vec<_>>();
    if segments.iter().any(|segment| segment.is_empty()) {
        return Err(OperatorError::Platform(format!(
            "model field path `{path}` contains an empty segment"
        )));
    }

    match segments.as_slice() {
        ["api_key"] => Ok(ModelConfigFieldPath::ApiKey),
        ["base_url"] => Ok(ModelConfigFieldPath::BaseUrl),
        ["model_name"] => Ok(ModelConfigFieldPath::ModelName),
        ["agent", ..] | ["provider", ..] => Err(OperatorError::Platform(format!(
            "model field path `{path}` must be relative to a single [agent.model.provider.<name>] entry"
        ))),
        _ => Err(OperatorError::Platform(format!(
            "model field path `{path}` is not part of the supported provider contract"
        ))),
    }
}

fn parse_toml_value(raw: &str) -> Result<TomlValue, OperatorError> {
    let mut table = toml::from_str::<toml::Table>(&format!("value = {raw}")).map_err(|error| {
        OperatorError::Platform(format!(
            "config set value `{raw}` could not be parsed: {error}"
        ))
    })?;
    table.remove("value").ok_or_else(|| {
        OperatorError::Platform(format!("config set value `{raw}` could not be parsed"))
    })
}

fn set_optional_string_field(table: &mut Table, key: &str, value: Option<&str>) {
    match value {
        Some(value) => table[key] = edit_value(value),
        None => {
            table.remove(key);
        }
    }
}

fn ensure_child_table<'a>(item: &'a mut Item, key: &str) -> &'a mut Table {
    if item.is_none() {
        *item = Item::Table(Table::new());
    }
    let table = item.as_table_mut().expect("table");
    if !table.contains_key(key) || !table[key].is_table() {
        table[key] = Item::Table(Table::new());
    }
    table[key].as_table_mut().expect("child table")
}

fn json_driver_config_to_item(config: &DriverConfig) -> Result<Item, OperatorError> {
    let mut table = Table::new();
    for (key, value) in config {
        table[key] = json_value_to_item(value)?;
    }
    Ok(Item::Table(table))
}

fn json_value_to_item(value: &JsonValue) -> Result<Item, OperatorError> {
    let toml_value = json_value_to_toml(value)?;
    toml_value_to_item(&toml_value)
}

fn json_value_to_toml(value: &JsonValue) -> Result<TomlValue, OperatorError> {
    match value {
        JsonValue::Null => Err(OperatorError::Platform(
            "driver_config values cannot contain null".into(),
        )),
        JsonValue::Bool(boolean) => Ok(TomlValue::Boolean(*boolean)),
        JsonValue::Number(number) => {
            if let Some(integer) = number.as_i64() {
                Ok(TomlValue::Integer(integer))
            } else if let Some(float) = number.as_f64() {
                Ok(TomlValue::Float(float))
            } else {
                Err(OperatorError::Platform(format!(
                    "unsupported numeric driver_config value `{number}`"
                )))
            }
        }
        JsonValue::String(string) => Ok(TomlValue::String(string.clone())),
        JsonValue::Array(values) => Ok(TomlValue::Array(
            values
                .iter()
                .map(json_value_to_toml)
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .collect(),
        )),
        JsonValue::Object(values) => {
            let mut table = toml::map::Map::new();
            for (key, value) in values {
                table.insert(key.clone(), json_value_to_toml(value)?);
            }
            Ok(TomlValue::Table(table))
        }
    }
}

fn toml_value_to_item(value: &TomlValue) -> Result<Item, OperatorError> {
    #[derive(Serialize)]
    struct ValueEnvelope<'a> {
        value: &'a TomlValue,
    }

    let rendered = toml::to_string(&ValueEnvelope { value }).map_err(|error| {
        OperatorError::Platform(format!(
            "failed to encode TOML value for config edit: {error}"
        ))
    })?;
    let mut document = rendered.parse::<DocumentMut>().map_err(|error| {
        OperatorError::Platform(format!(
            "failed to encode TOML value for config edit: {error}"
        ))
    })?;
    Ok(document
        .as_table_mut()
        .remove("value")
        .expect("serialized envelope should contain value"))
}

fn remove_nested_key(item: &mut Item, segments: &[String]) {
    if segments.is_empty() {
        return;
    }
    let Some(table) = item.as_table_mut() else {
        return;
    };
    if segments.len() == 1 {
        table.remove(&segments[0]);
        return;
    }
    if let Some(child) = table.get_mut(&segments[0]) {
        remove_nested_key(child, &segments[1..]);
        prune_empty_tables(child);
        if child.as_table().is_some_and(Table::is_empty) {
            table.remove(&segments[0]);
        }
    }
}

fn prune_empty_tables(item: &mut Item) {
    let Some(table) = item.as_table_mut() else {
        return;
    };
    let keys = table
        .iter()
        .map(|(key, _)| key.to_string())
        .collect::<Vec<_>>();
    for key in keys {
        if let Some(child) = table.get_mut(&key) {
            prune_empty_tables(child);
            if child.as_table().is_some_and(Table::is_empty) {
                table.remove(&key);
            }
        }
    }
}

fn render_target_path(path: &TargetConfigFieldPath) -> String {
    match path {
        TargetConfigFieldPath::Platform => "platform".into(),
        TargetConfigFieldPath::Driver => "driver".into(),
        TargetConfigFieldPath::Description => "description".into(),
        TargetConfigFieldPath::DriverConfig(segments) => {
            format!("driver_config.{}", segments.join("."))
        }
    }
}

fn render_model_path(path: &ModelConfigFieldPath) -> String {
    match path {
        ModelConfigFieldPath::ApiKey => "api_key".into(),
        ModelConfigFieldPath::BaseUrl => "base_url".into(),
        ModelConfigFieldPath::ModelName => "model_name".into(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        parse_model_set_expression, parse_target_set_expression, ModelConfigFieldPath,
        RuntimeConfigDocument, TargetConfigFieldPath,
    };
    use crate::runtime_config_path;
    use crate::AgentModelProviderConfig;
    use operator_core::TargetId;
    use operator_runtime::NamedTargetConfig;
    use serde_json::json;
    use toml::Value as TomlValue;

    #[test]
    fn target_set_expression_parses_typed_values_and_validates_paths() {
        let (path, value) = parse_target_set_expression("driver_config.addr='192.168.8.43:35319'")
            .expect("parse expression");
        assert_eq!(
            path,
            TargetConfigFieldPath::DriverConfig(vec!["addr".into()])
        );
        assert_eq!(value, TomlValue::String("192.168.8.43:35319".into()));

        let (path, value) =
            parse_target_set_expression("driver_config.retry_count=3").expect("parse int");
        assert_eq!(
            path,
            TargetConfigFieldPath::DriverConfig(vec!["retry_count".into()])
        );
        assert_eq!(value, TomlValue::Integer(3));

        let error = parse_target_set_expression("targets.harmony-pc.driver=harmony.hdc")
            .expect_err("absolute target path should fail");
        assert!(error
            .to_string()
            .contains("must be relative to a single [targets.<name>] entry"));
    }

    #[test]
    fn target_unset_path_rejects_required_fields() {
        let error =
            TargetConfigFieldPath::parse_unset("platform").expect_err("platform cannot be unset");
        assert!(error.to_string().contains("cannot be removed"));
    }

    #[test]
    fn model_set_expression_parses_string_values_and_validates_paths() {
        let (path, value) =
            parse_model_set_expression("api_key='sk-openai-1234'").expect("parse expression");
        assert_eq!(path, ModelConfigFieldPath::ApiKey);
        assert_eq!(value, TomlValue::String("sk-openai-1234".into()));

        let error = parse_model_set_expression("agent.model.provider.openai.api_key='value'")
            .expect_err("absolute model path should fail");
        assert!(error
            .to_string()
            .contains("must be relative to a single [agent.model.provider.<name>] entry"));
    }

    #[test]
    fn runtime_config_document_creates_missing_file_and_preserves_unrelated_sections() {
        let temp = tempdir().expect("tempdir");
        let path = runtime_config_path(temp.path());
        let mut document = RuntimeConfigDocument::load(&path).expect("load missing doc");
        document.set_default_target(&TargetId("windows-lab".into()));
        document
            .set_named_target(
                "windows-lab",
                &NamedTargetConfig {
                    platform: "windows".into(),
                    driver: "windows.remote".into(),
                    description: Some("Shared Windows lab".into()),
                    driver_config: [("endpoint".into(), json!("wss://lab.example"))].into(),
                },
            )
            .expect("set target");
        document.save().expect("save doc");

        let rendered = fs::read_to_string(&path).expect("read saved file");
        assert!(rendered.contains("default_target = \"windows-lab\""));
        assert!(rendered.contains("description = \"Shared Windows lab\""));
        assert!(rendered.contains("[targets.windows-lab.driver_config]"));
    }

    #[test]
    fn runtime_config_document_updates_target_fields_without_dropping_other_sections() {
        let temp = tempdir().expect("tempdir");
        let path = runtime_config_path(temp.path());
        fs::write(
            &path,
            r#"
[runtime]
default_target = "macos"

[targets.harmony-pc]
platform = "harmony"
driver = "harmony.hdc"

[targets.harmony-pc.driver_config]
addr = "192.168.8.43:35319"
agent_path = "/tmp/agent.so"

[agent]
model = "gpt-5.4"
"#,
        )
        .expect("write config");

        let mut document = RuntimeConfigDocument::load(&path).expect("load doc");
        document.set_default_target(&TargetId("harmony-pc".into()));
        document
            .set_target_value(
                "harmony-pc",
                &TargetConfigFieldPath::Description,
                TomlValue::String("Harmony lab PC".into()),
            )
            .expect("set description");
        document
            .unset_target_value(
                "harmony-pc",
                &TargetConfigFieldPath::DriverConfig(vec!["agent_path".into()]),
            )
            .expect("unset driver config");
        document.save().expect("save updated doc");

        let rendered = fs::read_to_string(&path).expect("read saved file");
        assert!(rendered.contains("[agent]"));
        assert!(rendered.contains("model = \"gpt-5.4\""));
        assert!(rendered.contains("description = \"Harmony lab PC\""));
        assert!(!rendered.contains("agent_path"));
        assert!(rendered.contains("addr = \"192.168.8.43:35319\""));
    }

    #[test]
    fn runtime_config_document_updates_model_fields_without_dropping_other_sections() {
        let temp = tempdir().expect("tempdir");
        let path = runtime_config_path(temp.path());
        fs::write(
            &path,
            r#"
[runtime]
default_target = "macos"

[targets.macos]
platform = "macos"
driver = "macos.system"

[agent]
max_steps = 50
step_timeout_ms = 30000
"#,
        )
        .expect("write config");

        let mut document = RuntimeConfigDocument::load(&path).expect("load doc");
        document
            .set_default_model_selector("openai")
            .expect("set selector");
        document
            .set_model_provider(
                "openai",
                &AgentModelProviderConfig {
                    api_key: Some("sk-openai-1234".into()),
                    base_url: Some("https://api.openai.com/v1".into()),
                    model_name: Some("gpt-5.4".into()),
                },
            )
            .expect("set provider");
        document
            .set_model_provider_value(
                "doubao",
                &ModelConfigFieldPath::BaseUrl,
                TomlValue::String("https://ark.cn-beijing.volces.com/api/v3".into()),
            )
            .expect("set doubao base_url");
        document
            .unset_model_provider_value("doubao", &ModelConfigFieldPath::BaseUrl)
            .expect("unset doubao field");
        document.save().expect("save updated doc");

        let rendered = fs::read_to_string(&path).expect("read saved file");
        assert!(rendered.contains("[agent]"));
        assert!(rendered.contains("max_steps = 50"));
        assert!(rendered.contains("step_timeout_ms = 30000"));
        assert!(rendered.contains("[agent.model]"));
        assert!(rendered.contains("default = \"openai\""));
        assert!(rendered.contains("[agent.model.provider.openai]"));
        assert!(rendered.contains("api_key = \"sk-openai-1234\""));
        assert!(rendered.contains("base_url = \"https://api.openai.com/v1\""));
        assert!(rendered.contains("model_name = \"gpt-5.4\""));
        assert!(!rendered.contains("[agent.model.provider.doubao]"));
    }

    #[test]
    fn runtime_config_document_lists_only_persisted_targets() {
        let temp = tempdir().expect("tempdir");
        let path = runtime_config_path(temp.path());
        let document = RuntimeConfigDocument::load(&path).expect("load missing doc");

        assert!(document.persisted_target_names().is_empty());
        assert!(!document.has_persisted_named_target("macos"));
    }

    #[test]
    fn runtime_config_document_rejects_driver_config_path_collisions() {
        let temp = tempdir().expect("tempdir");
        let path = runtime_config_path(temp.path());
        fs::write(
            &path,
            r#"
[runtime]
default_target = "harmony-pc"

[targets.harmony-pc]
platform = "harmony"
driver = "harmony.hdc"

[targets.harmony-pc.driver_config]
discovery = true
"#,
        )
        .expect("write config");

        let mut document = RuntimeConfigDocument::load(&path).expect("load doc");
        let error = document
            .set_target_value(
                "harmony-pc",
                &TargetConfigFieldPath::DriverConfig(vec!["discovery".into(), "enabled".into()]),
                TomlValue::Boolean(true),
            )
            .expect_err("path collision should fail");

        assert!(error
            .to_string()
            .contains("conflicts with existing non-table value at `driver_config.discovery`"));
    }
}
