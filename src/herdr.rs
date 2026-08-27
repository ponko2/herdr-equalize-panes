pub mod api;
pub mod env;
pub mod lock;
pub mod socket;
pub mod trigger;

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! herdr_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl From<String> for $name {
            fn from(id: String) -> Self {
                Self(id)
            }
        }

        impl From<&str> for $name {
            fn from(id: &str) -> Self {
                Self(id.to_owned())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

herdr_id!(PaneId);
herdr_id!(TabId);
herdr_id!(WorkspaceId);
