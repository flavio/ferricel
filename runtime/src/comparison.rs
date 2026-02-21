//! Comparison operations returning i64 booleans (1 for true, 0 for false).

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
