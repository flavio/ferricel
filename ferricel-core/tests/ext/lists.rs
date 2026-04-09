use crate::common::*;
use rstest::rstest;

#[rstest]
#[case::no_sep(r#"["a", "b", "c"].join()"#, "abc")]
#[case::with_sep(r#"["a", "b", "c"].join(",")"#, "a,b,c")]
#[case::empty_list(r#"[].join(",")"#, "")]
fn test_join(#[case] expr: &str, #[case] expected: &str) {
    let result = compile_and_execute_string(expr).expect("Failed to compile and execute");
    assert_eq!(
        result, expected,
        "Expression '{}' should evaluate to '{}'",
        expr, expected
    );
}
