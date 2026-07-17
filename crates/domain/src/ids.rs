use core::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! typed_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

typed_id!(ProjectId);
typed_id!(FileRecordId);
typed_id!(RunId);
typed_id!(TaskId);
typed_id!(AttemptId);
typed_id!(ContextVersionId);
typed_id!(ApiProfileId);

#[cfg(test)]
mod tests {
    use super::ProjectId;

    #[test]
    fn typed_ids_preserve_their_value_without_becoming_bare_strings() {
        let project_id = ProjectId::new("project-123");

        assert_eq!(project_id.as_str(), "project-123");
        assert_eq!(project_id.to_string(), "project-123");
    }
}
