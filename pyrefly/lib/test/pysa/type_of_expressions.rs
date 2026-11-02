/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::collections::HashMap;

use dupe::Dupe;

use crate::report::pysa::context::ModuleAnswersContext;
use crate::report::pysa::context::ModuleContext;
use crate::report::pysa::context::PysaResolver;
use crate::report::pysa::module::ModuleIds;
use crate::report::pysa::type_of_expression::export_type_of_expressions;
use crate::test::pysa::utils::create_state;
use crate::test::pysa::utils::get_handle_for_module_name;

/// The exported types of one function, keyed by expression location.
type Locations = HashMap<String, String>;

/// The expected `location -> type string` entries of one exported function.
type ExpectedLocations = Vec<(&'static str, &'static str)>;

fn type_of_expressions_for_test(
    module_name: &str,
    code: &str,
    all_expressions: bool,
) -> HashMap<String, Locations> {
    let state = create_state(module_name, code);
    let transaction = state.transaction();
    let handles = transaction.handles();
    let module_ids = ModuleIds::new(&handles);

    let test_module_handle = get_handle_for_module_name(module_name, &transaction);
    let resolver = PysaResolver::new_for_test(
        &transaction,
        &module_ids,
        test_module_handle.dupe(),
        &handles,
    );
    let context = ModuleContext {
        answers_context: ModuleAnswersContext::create(
            test_module_handle.dupe(),
            &transaction,
            &module_ids,
        ),
        resolver: &resolver,
    };

    export_type_of_expressions(&context, all_expressions)
        .into_iter()
        .map(|(function_id, data)| {
            let locations = data
                .locations
                .into_iter()
                .map(|(location, type_id)| {
                    (
                        location.as_key(),
                        data.type_table[type_id.0 as usize].string.clone(),
                    )
                })
                .collect();
            (function_id.serialize_to_string(), locations)
        })
        .collect()
}

fn assert_locations(actual: &HashMap<String, Locations>, expected: &[(&str, ExpectedLocations)]) {
    assert_eq!(actual.len(), expected.len(), "number of exported functions");
    for (function_id, expected_locations) in expected {
        let expected: Locations = expected_locations
            .iter()
            .map(|(location, ty)| ((*location).to_owned(), (*ty).to_owned()))
            .collect();
        assert_eq!(
            actual.get(*function_id),
            Some(&expected),
            "locations for function {function_id}"
        );
    }
}

const GREET: &str = r#"
def greet(name: str) -> str:
    return name.upper()

greeting = greet("hello")
"#;

#[test]
fn test_default_mode_exports_only_pysa_expressions() {
    let actual = type_of_expressions_for_test("test", GREET, false);
    assert_locations(
        &actual,
        &[
            (
                "MTL",
                vec![
                    // The callee `greet` and the call argument `"hello"`.
                    ("5:12-5:17", "(name: str) -> str"),
                    ("5:18-5:25", "str"),
                ],
            ),
            (
                "F:0",
                vec![
                    // The attribute base `name`.
                    ("3:12-3:16", "str"),
                ],
            ),
        ],
    );
}

#[test]
fn test_all_expressions_mode_exports_every_expression() {
    let actual = type_of_expressions_for_test("test", GREET, true);
    assert_locations(
        &actual,
        &[
            (
                "MTL",
                vec![
                    // The call `greet("hello")` and its callee `greet`.
                    ("5:12-5:26", "str"),
                    ("5:12-5:17", "(name: str) -> str"),
                    // The call argument `"hello"`.
                    ("5:18-5:25", "str"),
                ],
            ),
            (
                "F:0",
                vec![
                    // The return expression, the call `name.upper()` and the
                    // attribute access `name.upper`.
                    ("3:12-3:24", "str"),
                    ("3:12-3:24", "str"),
                    (
                        "3:12-3:22",
                        "Overload[\n  () -> typing.LiteralString\n  () -> str\n]",
                    ),
                    // The attribute base `name`.
                    ("3:12-3:16", "str"),
                ],
            ),
        ],
    );
}

#[test]
fn test_all_expressions_mode_exports_binary_operands() {
    let actual = type_of_expressions_for_test(
        "test",
        r#"
def add(a: int, b: int) -> int:
    return a + b

total = add(1, 2)
"#,
        true,
    );
    assert_locations(
        &actual,
        &[
            (
                "MTL",
                vec![
                    // The call `add(1, 2)`, its callee and its arguments.
                    ("5:9-5:18", "int"),
                    ("5:9-5:12", "(a: int, b: int) -> int"),
                    ("5:13-5:14", "int"),
                    ("5:16-5:17", "int"),
                ],
            ),
            (
                "F:0",
                vec![
                    // The return expression and the binary expression `a + b`.
                    ("3:12-3:17", "int"),
                    ("3:12-3:17", "int"),
                    // The operands `a` and `b`.
                    ("3:12-3:13", "int"),
                    ("3:16-3:17", "int"),
                ],
            ),
        ],
    );
}
