use serde::Serialize;
use std::mem;

extern crate alloc;
use alloc::vec::Vec;

// CEL Value representation for JSON serialization
#[derive(Serialize)]
#[serde(untagged)]
pub enum CelValue {
    Int(i64),
    Bool(bool),
    // Future: String(String), Double(f64), etc.
}

// 1. Memory Allocator for the Host
#[unsafe(no_mangle)]
pub extern "C" fn cel_malloc(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf);
    ptr
}

// Memory deallocator
#[unsafe(no_mangle)]
pub extern "C" fn cel_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len > 0 {
        unsafe {
            let _ = Vec::from_raw_parts(ptr, len, len);
            // Vec will be dropped here, freeing the memory
        }
    }
}

// 2. Helper functions for CEL logic (Exported so Walrus can find them)

// Arithmetic operations
// All operations use checked arithmetic to detect overflow and panic on error,
// matching CEL specification behavior
#[unsafe(no_mangle)]
pub extern "C" fn cel_int_add(a: i64, b: i64) -> i64 {
    a.checked_add(b).expect("integer overflow in addition")
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_sub(a: i64, b: i64) -> i64 {
    a.checked_sub(b).expect("integer overflow in subtraction")
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_mul(a: i64, b: i64) -> i64 {
    a.checked_mul(b)
        .expect("integer overflow in multiplication")
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_div(a: i64, b: i64) -> i64 {
    if b == 0 {
        panic!("division by zero");
    }
    // checked_div also catches the special case: i64::MIN / -1
    a.checked_div(b).expect("integer overflow in division")
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_mod(a: i64, b: i64) -> i64 {
    if b == 0 {
        panic!("modulo by zero");
    }
    // checked_rem also catches the special case: i64::MIN % -1
    a.checked_rem(b).expect("integer overflow in modulo")
}

// Comparison operations (return i64: 1 for true, 0 for false)
#[unsafe(no_mangle)]
pub extern "C" fn cel_int_eq(a: i64, b: i64) -> i64 {
    if a == b {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_ne(a: i64, b: i64) -> i64 {
    if a != b {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_gt(a: i64, b: i64) -> i64 {
    if a > b {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_lt(a: i64, b: i64) -> i64 {
    if a < b {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_gte(a: i64, b: i64) -> i64 {
    if a >= b {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_int_lte(a: i64, b: i64) -> i64 {
    if a <= b {
        1
    } else {
        0
    }
}

// Logical operations (work on i64 booleans, return i64)
#[unsafe(no_mangle)]
pub extern "C" fn cel_bool_and(a: i64, b: i64) -> i64 {
    if a != 0 && b != 0 {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_bool_or(a: i64, b: i64) -> i64 {
    if a != 0 || b != 0 {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cel_bool_not(a: i64) -> i64 {
    if a == 0 {
        1
    } else {
        0
    }
}

// 3. JSON Serialization Helpers

/// Encode pointer and length into a single i64
/// Low 32 bits = pointer, High 32 bits = length
#[inline]
fn encode_ptr_len(ptr: i32, len: i32) -> i64 {
    ((len as i64) << 32) | (ptr as i64 & 0xFFFFFFFF)
}

/// Serialize a CelValue to JSON and return (ptr, len) encoded in i64
fn serialize_to_json(value: &CelValue) -> i64 {
    // Serialize to JSON bytes
    let json_bytes = serde_json::to_vec(value).expect("Failed to serialize CelValue to JSON");

    let len = json_bytes.len();

    // Allocate memory for the JSON
    let ptr = cel_malloc(len);

    // Copy JSON bytes to allocated memory
    unsafe {
        std::ptr::copy_nonoverlapping(json_bytes.as_ptr(), ptr, len);
    }

    // Encode and return
    encode_ptr_len(ptr as i32, len as i32)
}

/// Convert i64 result to CelValue and serialize to JSON
/// Returns encoded (ptr, len) as i64
#[unsafe(no_mangle)]
pub extern "C" fn cel_serialize_int(value: i64) -> i64 {
    let cel_value = CelValue::Int(value);
    serialize_to_json(&cel_value)
}

/// Convert i64 boolean (0 or 1) to CelValue::Bool and serialize to JSON
/// Returns encoded (ptr, len) as i64
#[unsafe(no_mangle)]
pub extern "C" fn cel_serialize_bool(value: i64) -> i64 {
    let cel_value = CelValue::Bool(value != 0);
    serialize_to_json(&cel_value)
}
