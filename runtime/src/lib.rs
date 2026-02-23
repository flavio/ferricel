// Module declarations
mod arithmetic;
mod array;
mod comparison;
mod conversion;
mod deserialization;
mod field_access;
mod globals;
mod helpers;
mod logical;
mod memory;
mod serialization;
mod string;
mod types;

// Re-export public types
pub use types::CelValue;

// Re-export all WASM-callable functions
pub use memory::{cel_free, cel_malloc};

pub use arithmetic::{cel_int_div, cel_int_mod, cel_int_mul, cel_int_sub};

pub use comparison::{cel_int_eq, cel_int_gt, cel_int_gte, cel_int_lt, cel_int_lte, cel_int_ne};

pub use logical::{
    cel_bool_and, cel_bool_not, cel_bool_or, cel_conditional, cel_not_strictly_false,
};

pub use serialization::{cel_serialize_bool, cel_serialize_int, cel_serialize_value};

pub use deserialization::{cel_deserialize_json, cel_free_value};

pub use globals::{cel_get_data, cel_get_input, cel_init_data, cel_init_input, cel_reset_globals};

pub use conversion::{cel_value_to_bool, cel_value_to_i64};

pub use field_access::cel_get_field;

pub use array::{cel_array_get, cel_array_len, cel_array_push, cel_create_array};

pub use helpers::{cel_create_bool, cel_create_int, cel_value_add};

pub use string::{
    cel_create_string, cel_string_contains, cel_string_ends_with, cel_string_matches,
    cel_string_size, cel_string_starts_with,
};
