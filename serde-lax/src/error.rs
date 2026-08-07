use std::borrow::Cow;
use std::fmt;

use crate::path::{self, Path};

const MAX_DISPLAYED_ISSUES: usize = 100;

/// A decoding or JSON-syntax error.
///
/// Implements [`fmt::Display`] with one header line and up to the first 100
/// indented issue lines. [`Error::issues`] always contains the full list:
///
/// ```text
/// failed to decode into array of u64: 2 issues
///   at $[0]: expected u64, found string "1500"
///   at $[2]: expected u64, found null
/// ```
#[derive(Debug)]
pub struct Error {
    repr: Repr,
}

#[derive(Debug)]
enum Repr {
    Syntax(serde_json::Error),
    Decode {
        target: Cow<'static, str>,
        issues: Vec<Issue>,
    },
}

impl Error {
    pub(crate) fn syntax(error: serde_json::Error) -> Self {
        Error {
            repr: Repr::Syntax(error),
        }
    }

    pub(crate) fn decode(target: Cow<'static, str>, issues: Vec<Issue>) -> Self {
        Error {
            repr: Repr::Decode { target, issues },
        }
    }

    /// Every issue found while decoding, in deterministic traversal order:
    /// array elements come in index order; objects decoded through the map
    /// impls follow `serde_json`'s map iteration order (sorted by key, since
    /// `serde-lax` does not enable `preserve_order`); derived structs walk
    /// their fields in declaration order.
    ///
    /// Empty for syntax errors.
    #[must_use]
    pub fn issues(&self) -> &[Issue] {
        match &self.repr {
            Repr::Syntax(_) => &[],
            Repr::Decode { issues, .. } => issues,
        }
    }

    /// Whether the input was not valid JSON at all.
    #[must_use]
    pub fn is_syntax(&self) -> bool {
        matches!(self.repr, Repr::Syntax(_))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.repr {
            Repr::Syntax(error) => write!(f, "failed to parse JSON: {error}"),
            Repr::Decode { target, issues } => {
                let count = issues.len();
                let noun = if count == 1 { "issue" } else { "issues" };
                write!(f, "failed to decode into {target}: {count} {noun}")?;
                for issue in issues.iter().take(MAX_DISPLAYED_ISSUES) {
                    write!(f, "\n  at ")?;
                    match &issue.kind {
                        IssueKind::Mismatch { expected, found } => {
                            write!(f, "{}: expected {expected}, found {found}", issue.path)?;
                        }
                        IssueKind::MissingField { field, expected } => {
                            write!(f, "{}", issue.path)?;
                            path::write_key(f, field)?;
                            write!(f, ": missing required field (expected {expected})")?;
                        }
                        IssueKind::Custom { message } => {
                            write!(f, "{}: {message}", issue.path)?;
                        }
                    }
                }
                if count > MAX_DISPLAYED_ISSUES {
                    let hidden_count = count - MAX_DISPLAYED_ISSUES;
                    if hidden_count == 1 {
                        write!(f, "\n  … and 1 more issue (not shown)")?;
                    } else {
                        write!(f, "\n  … and {hidden_count} more issues (not shown)")?;
                    }
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.repr {
            Repr::Syntax(error) => Some(error),
            Repr::Decode { .. } => None,
        }
    }
}

/// One problem found while decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Where in the JSON document the problem was found.
    pub path: Path,
    /// What went wrong.
    pub kind: IssueKind,
}

/// The kind of problem an [`Issue`] describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueKind {
    /// The value at the path did not have the expected shape.
    Mismatch {
        /// What the target type expected, e.g. `u64`.
        expected: Cow<'static, str>,
        /// A description of the value actually found, e.g. `string "abc"`.
        found: String,
    },
    /// A required object field was absent.
    MissingField {
        /// The name of the missing field.
        field: String,
        /// What the field's type expected.
        expected: Cow<'static, str>,
    },
    /// A free-form issue recorded via [`crate::Context::custom`].
    Custom {
        /// The message, verbatim.
        message: String,
    },
}

/// Builds the `found` description for a JSON value.
pub(crate) fn describe_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_owned(),
        serde_json::Value::Bool(b) => format!("boolean {b}"),
        serde_json::Value::Number(n) => format!("number {n}"),
        serde_json::Value::String(s) => {
            let char_count = s.chars().count();
            if char_count > 40 {
                let truncated: String = s.chars().take(40).collect();
                format!("string {:?}", format!("{truncated}…"))
            } else {
                format!("string {s:?}")
            }
        }
        serde_json::Value::Array(items) => format!("array (len {})", items.len()),
        serde_json::Value::Object(entries) => {
            if entries.is_empty() {
                "object (empty)".to_owned()
            } else {
                let mut keys = entries
                    .keys()
                    .take(5)
                    .map(|key| format!("{key:?}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                if entries.len() > 5 {
                    keys.push_str(", …");
                }
                format!("object with keys {keys} ({} total)", entries.len())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use serde_json::json;

    use super::{describe_value, Error};
    use crate::Context;

    #[test]
    fn missing_field_lines_append_the_field_to_the_path() {
        let mut cx = Context::new();
        cx.with_key("customer", |cx| {
            cx.missing_field("id", "u64");
            cx.missing_field("full name", "string");
        });
        let err = Error::decode(Cow::Borrowed("Customer object"), cx.into_issues());
        assert_eq!(
            err.to_string(),
            "failed to decode into Customer object: 2 issues\n  at $.customer.id: missing required field (expected u64)\n  at $.customer[\"full name\"]: missing required field (expected string)"
        );
    }

    #[test]
    fn custom_lines_render_the_message_verbatim() {
        let mut cx = Context::new();
        cx.with_index(1, |cx| cx.custom("timestamp is in the future"));
        let err = Error::decode(Cow::Borrowed("array of Event"), cx.into_issues());
        assert_eq!(
            err.to_string(),
            "failed to decode into array of Event: 1 issue\n  at $[1]: timestamp is in the future"
        );
    }

    fn mismatched_array(issue_count: usize) -> Error {
        let input = format!("[{}]", vec![r#""bad""#; issue_count].join(","));
        crate::from_str::<Vec<u64>>(&input).expect_err("array elements must mismatch")
    }

    #[test]
    fn display_renders_all_one_hundred_issues_without_summary() {
        let err = mismatched_array(100);
        let rendered = err.to_string();
        let lines = rendered.lines().collect::<Vec<_>>();

        assert_eq!(err.issues().len(), 100);
        assert_eq!(lines.len(), 101);
        assert_eq!(lines[0], "failed to decode into array of u64: 100 issues");
        assert_eq!(
            lines.last().copied(),
            Some("  at $[99]: expected u64, found string \"bad\"")
        );
        assert!(!rendered.contains("not shown"));
    }

    #[test]
    fn display_summarizes_one_issue_over_the_cap() {
        let err = mismatched_array(101);
        let lines = err
            .to_string()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        assert_eq!(err.issues().len(), 101);
        assert_eq!(lines.len(), 102);
        assert_eq!(lines[0], "failed to decode into array of u64: 101 issues");
        assert_eq!(
            lines.last().map(String::as_str),
            Some("  … and 1 more issue (not shown)")
        );
    }

    #[test]
    fn display_summarizes_multiple_issues_over_the_cap() {
        let err = mismatched_array(102);
        let lines = err
            .to_string()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        assert_eq!(err.issues().len(), 102);
        assert_eq!(lines[0], "failed to decode into array of u64: 102 issues");
        assert_eq!(
            lines.last().map(String::as_str),
            Some("  … and 2 more issues (not shown)")
        );
    }

    #[test]
    fn describes_primitives() {
        assert_eq!(describe_value(&json!(null)), "null");
        assert_eq!(describe_value(&json!(true)), "boolean true");
        assert_eq!(describe_value(&json!(300)), "number 300");
        assert_eq!(describe_value(&json!(1.5)), "number 1.5");
        assert_eq!(describe_value(&json!(-3)), "number -3");
        assert_eq!(describe_value(&json!("abc")), "string \"abc\"");
    }

    #[test]
    fn long_strings_are_truncated_to_forty_chars() {
        let long = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        assert_eq!(
            describe_value(&json!(long)),
            "string \"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx…\""
        );
    }

    #[test]
    fn forty_char_strings_are_not_truncated() {
        let exact = "yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy";
        assert_eq!(
            describe_value(&json!(exact)),
            "string \"yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy\""
        );
    }

    #[test]
    fn describes_arrays_by_length() {
        assert_eq!(describe_value(&json!([1, 2, 3])), "array (len 3)");
        assert_eq!(describe_value(&json!([])), "array (len 0)");
    }

    #[test]
    fn describes_objects_by_keys() {
        assert_eq!(describe_value(&json!({})), "object (empty)");
        assert_eq!(
            describe_value(&json!({"a": 1, "b": 2})),
            "object with keys \"a\", \"b\" (2 total)"
        );
    }

    #[test]
    fn large_objects_list_only_five_keys() {
        let value = json!({"a": 1, "b": 2, "c": 3, "d": 4, "e": 5, "f": 6, "g": 7});
        assert_eq!(
            describe_value(&value),
            "object with keys \"a\", \"b\", \"c\", \"d\", \"e\", … (7 total)"
        );
    }
}
