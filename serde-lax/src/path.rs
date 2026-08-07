use std::fmt;

/// One step into a JSON document: an object key or an array index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// An object key, as in `$.customer`.
    Key(String),
    /// An array index, as in `$[2]`.
    Index(usize),
}

/// A path from the JSON document root to the place an issue was found.
///
/// Renders as `$` for the root, `.key` for identifier-like keys, `["quoted"]`
/// for any other key, and `[index]` for array elements — e.g.
/// `$.customer.tags[2]` or `$["weird key"].id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path(pub(crate) Vec<Segment>);

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "$")?;
        for segment in &self.0 {
            match segment {
                Segment::Key(key) => write_key(f, key)?,
                Segment::Index(index) => write!(f, "[{index}]")?,
            }
        }
        Ok(())
    }
}

/// Writes a key segment: `.key` when the key looks like an identifier,
/// `["quoted"]` otherwise.
pub(crate) fn write_key(f: &mut fmt::Formatter<'_>, key: &str) -> fmt::Result {
    if is_identifier(key) {
        write!(f, ".{key}")
    } else {
        write!(f, "[{key:?}]")
    }
}

fn is_identifier(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::{Path, Segment};

    #[test]
    fn root_renders_as_dollar() {
        assert_eq!(Path(Vec::new()).to_string(), "$");
    }

    #[test]
    fn identifier_keys_use_dot_notation() {
        let path = Path(vec![
            Segment::Key("customer".to_owned()),
            Segment::Key("tags".to_owned()),
            Segment::Index(2),
        ]);
        assert_eq!(path.to_string(), "$.customer.tags[2]");
    }

    #[test]
    fn non_identifier_keys_are_quoted() {
        let path = Path(vec![
            Segment::Key("weird key".to_owned()),
            Segment::Key("id".to_owned()),
        ]);
        assert_eq!(path.to_string(), "$[\"weird key\"].id");
    }

    #[test]
    fn empty_and_digit_leading_keys_are_quoted() {
        let path = Path(vec![
            Segment::Key(String::new()),
            Segment::Key("0ops".to_owned()),
        ]);
        assert_eq!(path.to_string(), "$[\"\"][\"0ops\"]");
    }
}
