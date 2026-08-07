use std::borrow::Cow;

use crate::error::{describe_value, Issue, IssueKind};
use crate::path::{Path, Segment};

/// Decoding state: the current JSON path and every issue recorded so far.
///
/// Entry points ([`crate::from_value`] and friends) build the context;
/// [`crate::FromJson`] implementations only use the methods below.
pub struct Context {
    path: Vec<Segment>,
    issues: Vec<Issue>,
}

impl Context {
    pub(crate) fn new() -> Self {
        Context {
            path: Vec::new(),
            issues: Vec::new(),
        }
    }

    /// Runs `f` with the object key `key` pushed onto the current path.
    ///
    /// The segment is popped even if `f` panics, so a caught panic cannot
    /// corrupt the paths of issues recorded afterwards.
    pub fn with_key<T>(&mut self, key: &str, f: impl FnOnce(&mut Context) -> T) -> T {
        self.with_segment(Segment::Key(key.to_owned()), f)
    }

    /// Runs `f` with the array index `index` pushed onto the current path.
    ///
    /// The segment is popped even if `f` panics, so a caught panic cannot
    /// corrupt the paths of issues recorded afterwards.
    pub fn with_index<T>(&mut self, index: usize, f: impl FnOnce(&mut Context) -> T) -> T {
        self.with_segment(Segment::Index(index), f)
    }

    fn with_segment<T>(&mut self, segment: Segment, f: impl FnOnce(&mut Context) -> T) -> T {
        struct PopOnDrop<'a> {
            cx: &'a mut Context,
        }

        impl Drop for PopOnDrop<'_> {
            fn drop(&mut self) {
                self.cx.path.pop();
            }
        }

        self.path.push(segment);
        let guard = PopOnDrop { cx: self };
        f(guard.cx)
    }

    /// Records a type mismatch at the current path: `expected` describes the
    /// target type, `found` is the JSON value that was actually there.
    pub fn mismatch(&mut self, expected: impl Into<Cow<'static, str>>, found: &serde_json::Value) {
        self.push(IssueKind::Mismatch {
            expected: expected.into(),
            found: describe_value(found),
        });
    }

    /// Records a missing required field at the current path plus `field`.
    pub fn missing_field(&mut self, field: &str, expected: impl Into<Cow<'static, str>>) {
        self.push(IssueKind::MissingField {
            field: field.to_owned(),
            expected: expected.into(),
        });
    }

    /// Records a free-form issue at the current path.
    pub fn custom(&mut self, message: impl Into<String>) {
        self.push(IssueKind::Custom {
            message: message.into(),
        });
    }

    /// How many issues have been recorded so far.
    #[must_use]
    pub fn issue_count(&self) -> usize {
        self.issues.len()
    }

    pub(crate) fn into_issues(self) -> Vec<Issue> {
        self.issues
    }

    fn push(&mut self, kind: IssueKind) {
        self.issues.push(Issue {
            path: Path(self.path.clone()),
            kind,
        });
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Context;

    #[test]
    fn path_is_restored_after_scoped_calls() {
        let mut cx = Context::new();
        cx.with_key("outer", |cx| {
            cx.with_index(3, |cx| cx.mismatch("u64", &json!(null)));
        });
        cx.mismatch("bool", &json!(1));
        let issues = cx.into_issues();
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].path.to_string(), "$.outer[3]");
        assert_eq!(issues[1].path.to_string(), "$");
    }

    #[test]
    fn path_is_restored_when_a_scoped_closure_panics() {
        let mut cx = Context::new();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cx.with_key("abandoned", |_cx| panic!("boom"));
        }));
        assert!(outcome.is_err());
        cx.custom("recorded after the panic");
        let issues = cx.into_issues();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].path.to_string(), "$");
    }
}
