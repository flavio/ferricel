//! Boolean logic operations on i64 values (0 = false, non-zero = true).

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
