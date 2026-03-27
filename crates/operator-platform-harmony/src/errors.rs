use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HarmonyConfigError {
    #[error("missing harmony.hdc driver_config field `{field}`")]
    MissingField { field: &'static str },

    #[error("invalid harmony.hdc driver_config field `{field}`: expected {expected}")]
    InvalidField {
        field: &'static str,
        expected: &'static str,
    },

    #[error("unknown harmony.hdc driver_config field `{field}`")]
    UnknownField { field: String },
}

impl HarmonyConfigError {
    pub(crate) fn missing(field: &'static str) -> Self {
        Self::MissingField { field }
    }

    pub(crate) fn invalid(field: &'static str, expected: &'static str) -> Self {
        Self::InvalidField { field, expected }
    }

    pub(crate) fn unknown(field: impl Into<String>) -> Self {
        Self::UnknownField {
            field: field.into(),
        }
    }
}

pub(crate) fn hdc_platform_error(
    context: &str,
    error: impl std::fmt::Display,
) -> operator_core::OperatorError {
    operator_core::OperatorError::Platform(format!("{context}: {error}"))
}
