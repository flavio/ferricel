// Module declarations
mod arithmetic;
mod array;
mod bytes;
mod chrono_helpers;
mod comparison;
mod conversion;
mod deserialization;
pub mod error;
mod extensions;
mod field_access;
mod globals;
mod helpers;
mod kubernetes;
pub mod logging;
mod logical;
mod map;
mod membership;
mod memory;
pub(crate) mod proto_wire;
mod serialization;
mod string;
mod temporal;
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
    cel_duration_gt, cel_duration_gte, cel_duration_lt, cel_duration_lte, cel_int_eq, cel_int_gt,
    cel_int_gte, cel_int_lt, cel_int_lte, cel_int_ne, cel_timestamp_gt, cel_timestamp_gte,
    cel_timestamp_lt, cel_timestamp_lte, cel_uint_eq, cel_uint_gt, cel_uint_gte, cel_uint_lt,
    cel_uint_lte, cel_uint_ne,
};

pub use logical::{
    cel_bool_and, cel_bool_not, cel_bool_or, cel_conditional, cel_is_bool_or_error, cel_is_error,
    cel_is_strictly_false, cel_is_strictly_true, cel_not_strictly_false,
};

pub use serialization::{cel_serialize_bool, cel_serialize_int, cel_serialize_value};

pub use deserialization::{cel_deserialize_json, cel_deserialize_proto, cel_free_value};

pub use globals::{cel_get_variable, cel_init_bindings, cel_reset_globals};

pub use conversion::{
    cel_bool, cel_bytes, cel_double, cel_duration, cel_int, cel_string, cel_timestamp, cel_type,
    cel_uint, cel_value_to_bool, cel_value_to_i64, cel_value_to_u64,
};

pub use field_access::{cel_get_field, cel_has_field};

pub use array::{cel_array_get, cel_array_len, cel_array_push, cel_create_array};

pub use map::{cel_create_map, cel_map_insert};

pub use helpers::{
    cel_create_bool, cel_create_double, cel_create_duration, cel_create_error, cel_create_int,
    cel_create_null, cel_create_timestamp, cel_create_type, cel_create_uint, cel_value_add,
    cel_value_div, cel_value_eq, cel_value_gt, cel_value_gte, cel_value_index, cel_value_lt,
    cel_value_lte, cel_value_mod, cel_value_mul, cel_value_ne, cel_value_negate, cel_value_size,
    cel_value_sub,
};

pub use string::{
    cel_create_string, cel_string_contains, cel_string_ends_with, cel_string_matches,
    cel_string_size, cel_string_starts_with,
};

pub use bytes::{
    cel_bytes_concat, cel_bytes_eq, cel_bytes_gt, cel_bytes_gte, cel_bytes_lt, cel_bytes_lte,
    cel_bytes_ne, cel_bytes_size, cel_create_bytes,
};

pub use membership::cel_value_in;

pub use temporal::{
    cel_duration_add, cel_duration_negate, cel_duration_sub, cel_timestamp_add_duration,
    cel_timestamp_diff, cel_timestamp_get_date, cel_timestamp_get_date_tz,
    cel_timestamp_get_day_of_month, cel_timestamp_get_day_of_month_tz,
    cel_timestamp_get_day_of_week, cel_timestamp_get_day_of_week_tz, cel_timestamp_get_day_of_year,
    cel_timestamp_get_day_of_year_tz, cel_timestamp_get_full_year, cel_timestamp_get_full_year_tz,
    cel_timestamp_get_hours, cel_timestamp_get_hours_tz, cel_timestamp_get_milliseconds,
    cel_timestamp_get_milliseconds_tz, cel_timestamp_get_minutes, cel_timestamp_get_minutes_tz,
    cel_timestamp_get_month, cel_timestamp_get_month_tz, cel_timestamp_get_seconds,
    cel_timestamp_get_seconds_tz, cel_timestamp_sub_duration,
};

// Re-export logging functions
pub use logging::cel_set_log_level;

// Re-export extension call wrappers
pub use extensions::{
    cel_ext_call_0, cel_ext_call_1, cel_ext_call_2, cel_ext_call_3, cel_ext_call_4,
};

// Re-export Kubernetes list extension functions
pub use kubernetes::lists::{
    cel_k8s_list_index_of, cel_k8s_list_is_sorted, cel_k8s_list_last_index_of, cel_k8s_list_max,
    cel_k8s_list_min, cel_k8s_list_sum,
};

// Re-export Kubernetes regex extension functions
pub use kubernetes::regex::{cel_k8s_regex_find, cel_k8s_regex_find_all_n};

// Re-export Kubernetes URL extension functions
pub use kubernetes::url::{
    cel_k8s_is_url, cel_k8s_url_get_escaped_path, cel_k8s_url_get_host, cel_k8s_url_get_hostname,
    cel_k8s_url_get_port, cel_k8s_url_get_query, cel_k8s_url_get_scheme, cel_k8s_url_parse,
};
