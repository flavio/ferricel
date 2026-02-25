// Module declarations
mod arithmetic;
mod array;
mod comparison;
mod conversion;
mod deserialization;
mod field_access;
mod globals;
mod helpers;
pub mod logging;
mod logical;
mod map;
mod membership;
mod memory;
mod serialization;
mod string;
mod types;

// Re-export public types
pub use types::CelValue;

// Re-export all WASM-callable functions
pub use memory::{cel_free, cel_malloc};

pub use arithmetic::{
    cel_double_add, cel_double_div, cel_double_mul, cel_double_sub, cel_int_div, cel_int_mod,
    cel_int_mul, cel_int_sub, cel_uint_add, cel_uint_div, cel_uint_mod, cel_uint_mul, cel_uint_sub,
};

pub use comparison::{
    cel_double_eq, cel_double_gt, cel_double_gte, cel_double_lt, cel_double_lte, cel_double_ne,
    cel_int_eq, cel_int_gt, cel_int_gte, cel_int_lt, cel_int_lte, cel_int_ne, cel_uint_eq,
    cel_uint_gt, cel_uint_gte, cel_uint_lt, cel_uint_lte, cel_uint_ne,
};

pub use logical::{
    cel_bool_and, cel_bool_not, cel_bool_or, cel_conditional, cel_not_strictly_false,
};

pub use serialization::{cel_serialize_bool, cel_serialize_int, cel_serialize_value};

pub use deserialization::{cel_deserialize_json, cel_free_value};

pub use globals::{cel_get_data, cel_get_input, cel_init_data, cel_init_input, cel_reset_globals};

pub use conversion::{
    cel_double, cel_int, cel_uint, cel_value_to_bool, cel_value_to_i64, cel_value_to_u64,
};

pub use field_access::cel_get_field;

pub use array::{cel_array_get, cel_array_len, cel_array_push, cel_create_array};

pub use map::{cel_create_map, cel_map_insert};

pub use helpers::{
    cel_create_bool, cel_create_double, cel_create_int, cel_create_uint, cel_value_add,
    cel_value_div, cel_value_eq, cel_value_gt, cel_value_gte, cel_value_lt, cel_value_lte,
    cel_value_mod, cel_value_mul, cel_value_ne, cel_value_sub,
};

pub use string::{
    cel_create_string, cel_string_contains, cel_string_ends_with, cel_string_matches,
    cel_string_size, cel_string_starts_with,
};

pub use membership::cel_value_in;

// Re-export logging functions
pub use logging::cel_set_log_level;
