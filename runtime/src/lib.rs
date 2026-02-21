use std::mem;

// 1. Memory Allocator for the Host
#[unsafe(no_mangle)]
pub extern "C" fn cel_malloc(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf);
    ptr
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
