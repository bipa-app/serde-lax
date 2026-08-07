#[test]
fn derive_rejects_unsupported_shapes_and_attributes() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
