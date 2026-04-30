// Integration tests for list literals and list macros (all, exists, exists_one, filter, map).

mod common;
use common::*;

use rstest::rstest;

// ========================================
// List Literal Tests
// ========================================

#[rstest]
#[case::empty("[]", serde_json::json!([]))]
#[case::single_element("[42]", serde_json::json!([42]))]
#[case::multiple_integers("[1, 2, 3]", serde_json::json!([1, 2, 3]))]
#[case::with_expressions("[1 + 1, 2 * 3, 10 - 5]", serde_json::json!([2, 6, 5]))]
#[case::mixed_types("[1, true, 3, false]", serde_json::json!([1, true, 3, false]))]
#[case::with_comparisons("[5 > 3, 2 < 1, 10 == 10]", serde_json::json!([true, false, true]))]
#[case::concatenation("[1, 2] + [3, 4]", serde_json::json!([1, 2, 3, 4]))]
#[case::concatenation_empty("[] + []", serde_json::json!([]))]
#[case::concatenation_with_empty("[1, 2, 3] + []", serde_json::json!([1, 2, 3]))]
fn test_list_literals(#[case] expr: &str, #[case] expected: serde_json::Value) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(result, expected);
}

// ========================================
// all() Macro Tests
// ========================================

#[rstest]
#[case::all_true("[1, 2, 3].all(x, x > 0)", true)]
#[case::some_false("[1, -2, 3].all(x, x > 0)", false)]
#[case::empty_list("[].all(x, x > 0)", true)]
#[case::complex_predicate("[10, 20, 30].all(x, x >= 10 && x <= 30)", true)]
#[case::equality("[5, 5, 5].all(x, x == 5)", true)]
#[case::single_false("[1, 2, 3, 0].all(x, x > 0)", false)]
#[case::with_expressions("[1+1, 2*3, 10-5].all(x, x > 1)", true)]
fn test_all_macro(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(result, serde_json::json!(expected));
}

// ========================================
// exists() Macro Tests
// ========================================

#[rstest]
#[case::one_true("[1, 2, 3].exists(x, x > 2)", true)]
#[case::all_false("[1, 2, 3].exists(x, x > 10)", false)]
#[case::empty_list("[].exists(x, x > 0)", false)]
#[case::all_true("[5, 10, 15].exists(x, x > 0)", true)]
#[case::complex_predicate("[1, 5, 10].exists(x, x >= 5 && x <= 10)", true)]
#[case::first_element_true("[10, 1, 2].exists(x, x > 5)", true)]
#[case::last_element_true("[1, 2, 10].exists(x, x > 5)", true)]
fn test_exists_macro(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(result, serde_json::json!(expected));
}

// ========================================
// exists_one() Macro Tests
// ========================================

#[rstest]
#[case::exactly_one("[1, 5, 3].exists_one(x, x > 4)", true)]
#[case::none("[1, 2, 3].exists_one(x, x > 10)", false)]
#[case::multiple("[5, 10, 15].exists_one(x, x > 4)", false)]
#[case::empty_list("[].exists_one(x, x > 0)", false)]
#[case::first_element_only("[10, 1, 2].exists_one(x, x > 5)", true)]
#[case::last_element_only("[1, 2, 10].exists_one(x, x > 5)", true)]
#[case::two_elements("[10, 20, 1].exists_one(x, x > 5)", false)]
fn test_exists_one_macro(#[case] expr: &str, #[case] expected: bool) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(result, serde_json::json!(expected));
}

// ========================================
// filter() Macro Tests
// ========================================

#[rstest]
#[case::basic("[1, 2, 3, 4, 5].filter(x, x > 2)", serde_json::json!([3, 4, 5]))]
#[case::none_match("[1, 2, 3].filter(x, x > 10)", serde_json::json!([]))]
#[case::all_match("[1, 2, 3].filter(x, x > 0)", serde_json::json!([1, 2, 3]))]
#[case::empty_list("[].filter(x, x > 0)", serde_json::json!([]))]
#[case::even_numbers("[1, 2, 3, 4, 5, 6].filter(x, x % 2 == 0)", serde_json::json!([2, 4, 6]))]
#[case::complex_predicate("[1, 5, 10, 15, 20].filter(x, x >= 5 && x <= 15)", serde_json::json!([5, 10, 15]))]
#[case::first_element_only("[10, 1, 2, 3].filter(x, x > 5)", serde_json::json!([10]))]
#[case::last_element_only("[1, 2, 3, 10].filter(x, x > 5)", serde_json::json!([10]))]
fn test_filter_macro(#[case] expr: &str, #[case] expected: serde_json::Value) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(result, expected);
}

// ========================================
// map() Macro Tests
// ========================================

#[rstest]
#[case::basic("[1, 2, 3].map(x, x * 2)", serde_json::json!([2, 4, 6]))]
#[case::empty_list("[].map(x, x * 2)", serde_json::json!([]))]
#[case::identity("[1, 2, 3].map(x, x)", serde_json::json!([1, 2, 3]))]
#[case::addition("[1, 2, 3].map(x, x + 10)", serde_json::json!([11, 12, 13]))]
#[case::square("[1, 2, 3, 4].map(x, x * x)", serde_json::json!([1, 4, 9, 16]))]
#[case::type_change("[1, 2, 3].map(x, x > 1)", serde_json::json!([false, true, true]))]
#[case::division("[10, 20, 30].map(x, x / 10)", serde_json::json!([1, 2, 3]))]
#[case::complex_expression("[1, 2, 3].map(x, (x * 2) + 1)", serde_json::json!([3, 5, 7]))]
#[case::single_element("[5].map(x, x * 2)", serde_json::json!([10]))]
#[case::negative_numbers("[-1, -2, -3].map(x, x * -1)", serde_json::json!([1, 2, 3]))]
#[case::modulo("[10, 11, 12].map(x, x % 3)", serde_json::json!([1, 2, 0]))]
fn test_map_macro(#[case] expr: &str, #[case] expected: serde_json::Value) {
    let result = compile_and_execute(expr).expect("Failed to compile and execute");
    assert_eq!(result, expected);
}
