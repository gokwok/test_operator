use std::{
    fmt::{self, Display, Formatter},
    path::{Component, Path},
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
        pub struct $name(pub String);

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_id!(SnapshotId);
string_id!(ElementId);
string_id!(SessionId);
string_id!(TargetId);
string_id!(ArtifactId);

impl ArtifactId {
    pub fn as_file_name(&self) -> Result<&str, crate::OperatorError> {
        if self.0.is_empty()
            || self.0.contains('/')
            || self.0.contains('\\')
            || self.0.contains('\0')
        {
            return Err(invalid_artifact_id(&self.0));
        }

        let mut components = Path::new(&self.0).components();
        match (components.next(), components.next()) {
            (Some(Component::Normal(_)), None) => Ok(self.0.as_str()),
            _ => Err(invalid_artifact_id(&self.0)),
        }
    }
}

fn invalid_artifact_id(value: &str) -> crate::OperatorError {
    crate::OperatorError::Platform(format!("invalid artifact id: {value}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct WindowId(pub u64);

impl From<u64> for WindowId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl Display for WindowId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
