use crate::common::*;
use rstest::rstest;

#[rstest]
#[case::basic(r#""tacocat".indexOf("ac")"#, 1_i64)]
#[case::not_found(r#""tacocat".indexOf("none")"#, -1_i64)]
#[case::empty_needle(r#""tacocat".indexOf("")"#, 0_i64)]
#[case::with_offset(r#""tacocat".indexOf("a", 3)"#, 5_i64)]
#[case::unicode(r#""ta©o©αT".indexOf("©")"#, 2_i64)]
fn test_index_of(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}

#[rstest]
#[case::basic(r#""tacocat".lastIndexOf("at")"#, 5_i64)]
#[case::not_found(r#""tacocat".lastIndexOf("none")"#, -1_i64)]
#[case::empty_needle(r#""tacocat".lastIndexOf("")"#, 7_i64)]
#[case::with_offset(r#""tacocat".lastIndexOf("a", 3)"#, 1_i64)]
fn test_last_index_of(#[case] expr: &str, #[case] expected: i64) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to {}",
        expr, expected
    );
}
