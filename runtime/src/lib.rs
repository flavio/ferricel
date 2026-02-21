extern crate alloc;

// Module declarations
mod arithmetic;
mod comparison;
mod logical;
mod memory;
mod serialization;
mod types;

// Re-export public types
pub use types::CelValue;

// Re-export all WASM-callable functions
pub use memory::{cel_free, cel_malloc};

pub use arithmetic::{cel_int_add, cel_int_div, cel_int_mod, cel_int_mul, cel_int_sub};

pub use comparison::{cel_int_eq, cel_int_gt, cel_int_gte, cel_int_lt, cel_int_lte, cel_int_ne};

pub use logical::{cel_bool_and, cel_bool_not, cel_bool_or};

pub use serialization::{cel_serialize_bool, cel_serialize_int};
