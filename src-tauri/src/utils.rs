use std::fmt::Write;

use serde::de::IgnoredAny;
use serde_json::value::RawValue;

use crate::Error;

pub fn serialize_as_json_array<'a, I: ExactSizeIterator<Item = &'a str>>(
    iter: I,
) -> Result<Box<RawValue>, Error> {
    let mut json = String::with_capacity(iter.len() + 2);
    json.push('[');

    for (i, col) in iter.enumerate() {
        if i > 0 {
            json.push(',');
        }
        write!(&mut json, "\"{}\"", col)?;
    }
    json.push(']');

    Ok(RawValue::from_string(json).unwrap())
}

pub fn is_json(input: &[u8]) -> bool {
    serde_json::from_slice::<IgnoredAny>(input).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_as_json_array() {
        let iter = ["a", "b", "c"];
        let json = serialize_as_json_array(iter.iter().copied()).unwrap();
        assert_eq!(serde_json::to_string(&json).unwrap(), r#"["a","b","c"]"#);

        let iter = ["a"];
        let json = serialize_as_json_array(iter.iter().copied()).unwrap();
        assert_eq!(serde_json::to_string(&json).unwrap(), r#"["a"]"#);

        let iter = [];
        let json = serialize_as_json_array(iter.iter().copied()).unwrap();
        assert_eq!(serde_json::to_string(&json).unwrap(), r#"[]"#);
    }

    #[test]
    fn test_is_json() {
        assert!(is_json(b"{}"));
        assert!(is_json(b"[]"));
        assert!(is_json(b"{\"a\": 1}"));
        assert!(is_json(b"[\"a\", 1]"));
        assert!(is_json(b"[\"a\", 1, true, false, null]"));

        assert!(!is_json(b"{]"));
        assert!(!is_json(b"{\"a\": 1"));
    }
}
