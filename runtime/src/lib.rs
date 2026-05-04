// Use a bump-pointer allocator for Wasm builds. `dealloc` is a no-op: all
// memory is released when the host drops the Wasm instance. This eliminates
// double-frees, use-after-free, and memory leaks by design.
#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOCATOR: lol_alloc::AssumeSingleThreaded<lol_alloc::LeakingAllocator> =
    unsafe { lol_alloc::AssumeSingleThreaded::new(lol_alloc::LeakingAllocator::new()) };

// Module declarations
mod arithmetic;
mod array;
mod bytes;
mod chrono_helpers;
mod comparison;
mod conversion;
mod deserialization;
pub mod error;
mod ext;
mod extensions;
mod field_access;
mod globals;
mod helpers;
mod iter;
mod kubernetes;
pub mod logging;
mod logical;
mod map;
mod membership;
mod memory;
pub mod optional;
pub(crate) mod proto_wire;
mod serialization;
mod string;
mod temporal;
mod types;

// Shared test helpers available to all test modules within this crate.
#[cfg(test)]
pub(crate) mod test_helpers {
    use crate::types::CelValue;

    pub(crate) fn make_val(v: CelValue) -> *mut CelValue {
        Box::into_raw(Box::new(v))
    }

    pub(crate) fn make_str(s: &str) -> *mut CelValue {
        make_val(CelValue::String(s.to_string()))
    }

    pub(crate) fn make_int(n: i64) -> *mut CelValue {
        make_val(CelValue::Int(n))
    }

    pub(crate) fn make_array(elements: Vec<CelValue>) -> *mut CelValue {
        make_val(CelValue::Array(elements))
    }

    pub(crate) fn read_val(ptr: *mut CelValue) -> CelValue {
        let v = unsafe { (*ptr).clone() };
        unsafe { drop(Box::from_raw(ptr)) };
        v
    }

    /// `optional.of(s)` — wraps a string in `CelValue::Optional(Some(...))`.
    pub(crate) fn some_str(s: &str) -> CelValue {
        CelValue::Optional(Some(Box::new(CelValue::String(s.to_string()))))
    }

    /// `optional.none()` — returns `CelValue::Optional(None)`.
    pub(crate) fn none() -> CelValue {
        CelValue::Optional(None)
    }

    /// Builds a `CelValue::Array` of strings from a slice of `&str`.
    pub(crate) fn strs(items: &[&str]) -> CelValue {
        CelValue::Array(
            items
                .iter()
                .map(|s| CelValue::String(s.to_string()))
                .collect(),
        )
    }
}
