//! Arithmetic operations with overflow checking and division-by-zero protection.
//! All operations panic on overflow or invalid operations per CEL spec.

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
