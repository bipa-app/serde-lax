//! Smoke tests for the serde-lax core, without the derive.

#[test]
fn happy_decode() {
    let numbers: Vec<u64> = serde_lax::from_str("[1, 2, 3]").expect("decodes");
    assert_eq!(numbers, vec![1, 2, 3]);
}

#[test]
fn error_decode_reports_every_issue_with_paths() {
    let err = serde_lax::from_str::<Vec<u64>>(r#"["1500", 2, null]"#).expect_err("must fail");
    assert!(!err.is_syntax());
    assert_eq!(err.issues().len(), 2);
    assert_eq!(
        err.to_string(),
        "failed to decode into array of u64: 2 issues\n  at $[0]: expected u64, found string \"1500\"\n  at $[2]: expected u64, found null"
    );
}
