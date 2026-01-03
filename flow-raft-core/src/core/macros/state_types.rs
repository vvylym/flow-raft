//! Macro for generating state types (marker types + enum + From impls)
//!
//! This macro generates:
//! - Marker types for each state (for phantom type parameters)
//! - An enum variant for serialization
//! - From implementations to convert between marker types and enum
//!
//! # Syntax
//! ```ignore
//! define_state_types! {
//!     EnumName {
//!         VariantWithoutFields => MarkerType,
//!         VariantWithFields { field_name: field_type } => MarkerTypeWithFields,
//!     }
//! }
//! ```

/// Defines state types: marker types + From impls
///
/// This macro generates marker types for each enum variant and From implementations
/// to convert from marker types to the enum. The enum itself must be defined separately.
///
/// # Syntax
/// ```ignore
/// define_state_types! {
///     EnumName {
///         VariantWithoutFields => MarkerType,
///         VariantWithFields { field_name: field_type } => MarkerTypeWithFields,
///     }
/// }
/// ```
///
/// # Example
/// ```ignore
/// enum MyState {
///     Pending,
///     Failed { error: String },
/// }
///
/// define_state_types! {
///     MyState {
///         Pending => MyStatePending,
///         Failed { error: String } => MyStateFailed,
///     }
/// }
/// ```
#[macro_export]
macro_rules! define_state_types {
    // Match variant with fields
    (
        $enum_name:ident {
            $(
                $variant:ident { $( $field:ident : $field_type:ty ),* $(,)? } => $marker_type:ident,
            )*
        }
    ) => {
        $(
            // Marker type with fields
            #[derive(Debug, Clone, PartialEq, Eq, Hash)]
            pub struct $marker_type {
                $( pub $field : $field_type ),*
            }

            impl From<&$marker_type> for $enum_name {
                fn from(state: &$marker_type) -> Self {
                    $enum_name::$variant {
                        $( $field : state.$field.clone() ),*
                    }
                }
            }
        )*
    };

    // Match variant without fields
    (
        $enum_name:ident {
            $(
                $variant:ident => $marker_type:ident,
            )*
        }
    ) => {
        $(
            // Marker type without fields (unit struct)
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
            pub struct $marker_type;

            impl From<&$marker_type> for $enum_name {
                fn from(_: &$marker_type) -> Self {
                    $enum_name::$variant
                }
            }
        )*
    };

    // Match mixed (with and without fields) - this is the main entry point
    (
        $enum_name:ident {
            $( $rest:tt )*
        }
    ) => {
        $crate::define_state_types!(@inner $enum_name { $( $rest )* });
    };

    // Internal helper to process mixed variants
    (
        @inner $enum_name:ident {
            $variant:ident { $( $field:ident : $field_type:ty ),* $(,)? } => $marker_type:ident,
            $( $rest:tt )*
        }
    ) => {
        // Marker type with fields
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        /// Marker type for $variant state
        pub struct $marker_type {
            $( /// $field field
              pub $field : $field_type ),*
        }

        impl From<&$marker_type> for $enum_name {
            fn from(state: &$marker_type) -> Self {
                $enum_name::$variant {
                    $( $field : state.$field.clone() ),*
                }
            }
        }

        $crate::define_state_types!(@inner $enum_name { $( $rest )* });
    };

    (
        @inner $enum_name:ident {
            $variant:ident => $marker_type:ident,
            $( $rest:tt )*
        }
    ) => {
        // Marker type without fields (unit struct)
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        /// Marker type for $variant state
        pub struct $marker_type;

        impl From<&$marker_type> for $enum_name {
            fn from(_: &$marker_type) -> Self {
                $enum_name::$variant
            }
        }

        $crate::define_state_types!(@inner $enum_name { $( $rest )* });
    };

    (
        @inner $enum_name:ident {
        }
    ) => {};
}
