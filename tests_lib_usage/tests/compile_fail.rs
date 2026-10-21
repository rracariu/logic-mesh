//! The block macros reject what they cannot expand with a compile error
//! spanned on the offending tokens, never a panic or a confusing error
//! from the generated code. Each case under `tests/ui` is compiled
//! against `logic-mesh` and must fail with the `.stderr` next to it.
//!
//! Regenerate the expected output after an intended change with
//! `TRYBUILD=overwrite cargo test -p logic-mesh-lib-test --test compile_fail`.

#[test]
fn macro_errors() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
