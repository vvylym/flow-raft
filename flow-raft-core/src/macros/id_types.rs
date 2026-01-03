//! Macro for generating strongly-typed ID types
//!
//! Generates a newtype wrapper around UUID with all necessary trait implementations.

/// Defines a strongly-typed ID type with all necessary trait implementations
///
/// # Example
/// ```ignore
/// define_id_type!(TaskId);
/// ```
#[macro_export]
macro_rules! define_id_type {
    ($name:ident, $id_prefix:expr) => {
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            serde::Serialize,
            serde::Deserialize,
        )]
        /// Unique identifier for a $desc
        pub struct $name(uuid::Uuid);

        impl $name {
            /// Creates a new ID from a string
            pub fn parse(s: impl AsRef<str>) -> Result<Self, uuid::Error> {
                Ok(Self(uuid::Uuid::parse_str(s.as_ref())?))
            }
        }

        impl Default for $name {
            #[inline]
            fn default() -> Self {
                Self(uuid::Uuid::new_v4())
            }
        }

        impl From<uuid::Uuid> for $name {
            #[inline]
            fn from(uuid: uuid::Uuid) -> Self {
                Self(uuid)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}:{}", $id_prefix, self.0)
            }
        }

        impl AsRef<uuid::Uuid> for $name {
            #[inline]
            fn as_ref(&self) -> &uuid::Uuid {
                &self.0
            }
        }
    };
}
