use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

use crate::{Context, FromJson};

fn narrow<S, T: TryFrom<S>>(n: S) -> Option<T> {
    T::try_from(n).ok()
}

macro_rules! impl_unsigned {
    ($($ty:ty),*) => {$(
        impl FromJson for $ty {
            fn expected() -> Cow<'static, str> {
                Cow::Borrowed(stringify!($ty))
            }

            fn from_json(value: &serde_json::Value, cx: &mut Context) -> Option<Self> {
                if let Some(v) = value.as_u64().and_then(narrow::<u64, Self>) {
                    return Some(v);
                }
                cx.mismatch(Self::expected(), value);
                None
            }
        }
    )*};
}

macro_rules! impl_signed {
    ($($ty:ty),*) => {$(
        impl FromJson for $ty {
            fn expected() -> Cow<'static, str> {
                Cow::Borrowed(stringify!($ty))
            }

            fn from_json(value: &serde_json::Value, cx: &mut Context) -> Option<Self> {
                if let Some(v) = value.as_i64().and_then(narrow::<i64, Self>) {
                    return Some(v);
                }
                cx.mismatch(Self::expected(), value);
                None
            }
        }
    )*};
}

impl_unsigned!(u8, u16, u32, u64, usize);
impl_signed!(i8, i16, i32, i64, isize);

impl FromJson for f64 {
    fn expected() -> Cow<'static, str> {
        Cow::Borrowed("f64")
    }

    fn from_json(value: &serde_json::Value, cx: &mut Context) -> Option<Self> {
        match value.as_f64() {
            Some(n) => Some(n),
            None => {
                cx.mismatch(Self::expected(), value);
                None
            }
        }
    }
}

impl FromJson for f32 {
    fn expected() -> Cow<'static, str> {
        Cow::Borrowed("f32")
    }

    fn from_json(value: &serde_json::Value, cx: &mut Context) -> Option<Self> {
        match value.as_f64() {
            Some(n) => {
                let narrowed = n as f32;
                if n.is_finite() && !narrowed.is_finite() {
                    cx.mismatch(Self::expected(), value);
                    None
                } else {
                    Some(narrowed)
                }
            }
            None => {
                cx.mismatch(Self::expected(), value);
                None
            }
        }
    }
}

impl FromJson for bool {
    fn expected() -> Cow<'static, str> {
        Cow::Borrowed("bool")
    }

    fn from_json(value: &serde_json::Value, cx: &mut Context) -> Option<Self> {
        match value.as_bool() {
            Some(b) => Some(b),
            None => {
                cx.mismatch(Self::expected(), value);
                None
            }
        }
    }
}

impl FromJson for String {
    fn expected() -> Cow<'static, str> {
        Cow::Borrowed("string")
    }

    fn from_json(value: &serde_json::Value, cx: &mut Context) -> Option<Self> {
        match value.as_str() {
            Some(s) => Some(s.to_owned()),
            None => {
                cx.mismatch(Self::expected(), value);
                None
            }
        }
    }
}

impl<T: FromJson> FromJson for Option<T> {
    fn expected() -> Cow<'static, str> {
        Cow::Owned(format!("optional {}", T::expected()))
    }

    fn from_json(value: &serde_json::Value, cx: &mut Context) -> Option<Self> {
        if value.is_null() {
            return Some(None);
        }
        T::from_json(value, cx).map(Some)
    }
}

impl<T: FromJson> FromJson for Vec<T> {
    fn expected() -> Cow<'static, str> {
        Cow::Owned(format!("array of {}", T::expected()))
    }

    fn from_json(value: &serde_json::Value, cx: &mut Context) -> Option<Self> {
        let Some(items) = value.as_array() else {
            cx.mismatch(Self::expected(), value);
            return None;
        };
        let mut out = Vec::with_capacity(items.len());
        let mut failed = false;
        for (index, item) in items.iter().enumerate() {
            match cx.with_index(index, |cx| T::from_json(item, cx)) {
                Some(v) => out.push(v),
                None => failed = true,
            }
        }
        if failed {
            None
        } else {
            Some(out)
        }
    }
}

macro_rules! impl_string_map {
    ($($map:ident),*) => {$(
        impl<V: FromJson> FromJson for $map<String, V> {
            fn expected() -> Cow<'static, str> {
                Cow::Owned(format!("object of {}", V::expected()))
            }

            fn from_json(value: &serde_json::Value, cx: &mut Context) -> Option<Self> {
                let Some(entries) = value.as_object() else {
                    cx.mismatch(Self::expected(), value);
                    return None;
                };
                let mut out = $map::new();
                let mut failed = false;
                for (key, item) in entries {
                    match cx.with_key(key, |cx| V::from_json(item, cx)) {
                        Some(v) => {
                            out.insert(key.clone(), v);
                        }
                        None => failed = true,
                    }
                }
                if failed {
                    None
                } else {
                    Some(out)
                }
            }
        }
    )*};
}

impl_string_map!(HashMap, BTreeMap);

impl<T: FromJson> FromJson for Box<T> {
    fn expected() -> Cow<'static, str> {
        T::expected()
    }

    fn from_json(value: &serde_json::Value, cx: &mut Context) -> Option<Self> {
        T::from_json(value, cx).map(Box::new)
    }
}

impl FromJson for serde_json::Value {
    fn expected() -> Cow<'static, str> {
        Cow::Borrowed("any JSON value")
    }

    fn from_json(value: &serde_json::Value, _cx: &mut Context) -> Option<Self> {
        Some(value.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use serde_json::json;

    use crate::{from_value, FromJson, IssueKind};

    fn decode_err<T: FromJson>(value: &serde_json::Value) -> crate::Error {
        match from_value::<T>(value) {
            Ok(_) => panic!("decode unexpectedly succeeded"),
            Err(err) => err,
        }
    }

    #[test]
    fn unsigned_integers_happy_and_mismatch() {
        assert_eq!(from_value::<u8>(&json!(255)).expect("decodes"), 255);
        assert_eq!(from_value::<u16>(&json!(65535)).expect("decodes"), 65535);
        assert_eq!(from_value::<u32>(&json!(7)).expect("decodes"), 7);
        assert_eq!(from_value::<u64>(&json!(1)).expect("decodes"), 1);
        assert_eq!(from_value::<usize>(&json!(9)).expect("decodes"), 9);

        assert_eq!(
            decode_err::<u8>(&json!("nope")).to_string(),
            "failed to decode into u8: 1 issue\n  at $: expected u8, found string \"nope\""
        );
        assert_eq!(
            decode_err::<u16>(&json!(true)).to_string(),
            "failed to decode into u16: 1 issue\n  at $: expected u16, found boolean true"
        );
        assert_eq!(
            decode_err::<u32>(&json!(null)).to_string(),
            "failed to decode into u32: 1 issue\n  at $: expected u32, found null"
        );
        assert_eq!(
            decode_err::<u64>(&json!([])).to_string(),
            "failed to decode into u64: 1 issue\n  at $: expected u64, found array (len 0)"
        );
        assert_eq!(
            decode_err::<usize>(&json!({})).to_string(),
            "failed to decode into usize: 1 issue\n  at $: expected usize, found object (empty)"
        );
    }

    #[test]
    fn signed_integers_happy_and_mismatch() {
        assert_eq!(from_value::<i8>(&json!(-128)).expect("decodes"), -128);
        assert_eq!(from_value::<i16>(&json!(-1)).expect("decodes"), -1);
        assert_eq!(from_value::<i32>(&json!(42)).expect("decodes"), 42);
        assert_eq!(from_value::<i64>(&json!(-9)).expect("decodes"), -9);
        assert_eq!(from_value::<isize>(&json!(0)).expect("decodes"), 0);

        assert_eq!(
            decode_err::<i8>(&json!("x")).to_string(),
            "failed to decode into i8: 1 issue\n  at $: expected i8, found string \"x\""
        );
        assert_eq!(
            decode_err::<i16>(&json!(false)).to_string(),
            "failed to decode into i16: 1 issue\n  at $: expected i16, found boolean false"
        );
        assert_eq!(
            decode_err::<i32>(&json!(null)).to_string(),
            "failed to decode into i32: 1 issue\n  at $: expected i32, found null"
        );
        assert_eq!(
            decode_err::<i64>(&json!(1.25)).to_string(),
            "failed to decode into i64: 1 issue\n  at $: expected i64, found number 1.25"
        );
        assert_eq!(
            decode_err::<isize>(&json!("z")).to_string(),
            "failed to decode into isize: 1 issue\n  at $: expected isize, found string \"z\""
        );
    }

    #[test]
    fn integer_out_of_range_reports_the_number() {
        assert_eq!(
            decode_err::<u8>(&json!(300)).to_string(),
            "failed to decode into u8: 1 issue\n  at $: expected u8, found number 300"
        );
        assert_eq!(
            decode_err::<i8>(&json!(200)).to_string(),
            "failed to decode into i8: 1 issue\n  at $: expected i8, found number 200"
        );
    }

    #[test]
    fn sixty_four_bit_extremes_decode() {
        assert_eq!(
            from_value::<u64>(&json!(18_446_744_073_709_551_615_u64)).expect("decodes"),
            u64::MAX
        );
        assert_eq!(
            from_value::<i64>(&json!(-9_223_372_036_854_775_808_i64)).expect("decodes"),
            i64::MIN
        );
        #[cfg(target_pointer_width = "64")]
        assert_eq!(
            from_value::<usize>(&json!(18_446_744_073_709_551_615_u64)).expect("decodes"),
            usize::MAX
        );
        #[cfg(target_pointer_width = "64")]
        assert_eq!(
            from_value::<isize>(&json!(-9_223_372_036_854_775_808_i64)).expect("decodes"),
            isize::MIN
        );
    }

    #[test]
    fn just_out_of_range_integers_are_rejected() {
        assert_eq!(
            decode_err::<u32>(&json!(4_294_967_296_u64)).to_string(),
            "failed to decode into u32: 1 issue\n  at $: expected u32, found number 4294967296"
        );
        assert_eq!(
            decode_err::<i32>(&json!(-2_147_483_649_i64)).to_string(),
            "failed to decode into i32: 1 issue\n  at $: expected i32, found number -2147483649"
        );
        assert_eq!(
            decode_err::<u8>(&json!(256)).to_string(),
            "failed to decode into u8: 1 issue\n  at $: expected u8, found number 256"
        );
        assert_eq!(
            decode_err::<i8>(&json!(-129)).to_string(),
            "failed to decode into i8: 1 issue\n  at $: expected i8, found number -129"
        );
    }

    #[test]
    fn narrow_integer_types_reject_floats_and_negatives() {
        assert_eq!(
            decode_err::<u16>(&json!(1.5)).to_string(),
            "failed to decode into u16: 1 issue\n  at $: expected u16, found number 1.5"
        );
        assert_eq!(
            decode_err::<i16>(&json!(1.5)).to_string(),
            "failed to decode into i16: 1 issue\n  at $: expected i16, found number 1.5"
        );
        assert_eq!(
            decode_err::<u16>(&json!(-1)).to_string(),
            "failed to decode into u16: 1 issue\n  at $: expected u16, found number -1"
        );
    }

    #[test]
    fn negative_number_for_unsigned_is_a_mismatch() {
        assert_eq!(
            decode_err::<u64>(&json!(-3)).to_string(),
            "failed to decode into u64: 1 issue\n  at $: expected u64, found number -3"
        );
    }

    #[test]
    fn float_where_integer_expected_is_a_mismatch() {
        assert_eq!(
            decode_err::<u64>(&json!(1.5)).to_string(),
            "failed to decode into u64: 1 issue\n  at $: expected u64, found number 1.5"
        );
    }

    #[test]
    fn floats_accept_representable_numbers() {
        assert_eq!(from_value::<f64>(&json!(1.5)).expect("decodes"), 1.5);
        assert_eq!(from_value::<f64>(&json!(3)).expect("decodes"), 3.0);
        assert_eq!(from_value::<f32>(&json!(2.5)).expect("decodes"), 2.5);
        assert_eq!(from_value::<f32>(&json!(-4)).expect("decodes"), -4.0);
        assert_eq!(from_value::<f32>(&json!(3.5)).expect("decodes"), 3.5);
        assert_eq!(from_value::<f32>(&json!(0.1)).expect("decodes"), 0.1);
        assert_eq!(from_value::<f64>(&json!(1e300)).expect("decodes"), 1e300);

        assert_eq!(
            decode_err::<f64>(&json!("1.5")).to_string(),
            "failed to decode into f64: 1 issue\n  at $: expected f64, found string \"1.5\""
        );
        assert_eq!(
            decode_err::<f32>(&json!(null)).to_string(),
            "failed to decode into f32: 1 issue\n  at $: expected f32, found null"
        );
    }

    #[test]
    fn f32_rejects_finite_numbers_that_overflow() {
        assert_eq!(
            decode_err::<f32>(&json!(1e300)).to_string(),
            "failed to decode into f32: 1 issue\n  at $: expected f32, found number 1e+300"
        );
        assert_eq!(
            decode_err::<f32>(&json!(-1e300)).to_string(),
            "failed to decode into f32: 1 issue\n  at $: expected f32, found number -1e+300"
        );
    }

    #[test]
    fn negative_zero_decodes_as_f64_and_not_i64() {
        let value: serde_json::Value = serde_json::from_str("-0").expect("valid JSON");
        assert_eq!(
            decode_err::<i64>(&value).to_string(),
            "failed to decode into i64: 1 issue\n  at $: expected i64, found number -0.0"
        );
        let decoded = from_value::<f64>(&value).expect("decodes");
        assert_eq!(decoded, -0.0);
        assert!(decoded.is_sign_negative());
    }

    #[test]
    fn exponent_forms_remain_floats() {
        let integer_exponent: serde_json::Value = serde_json::from_str("1e2").expect("valid JSON");
        assert_eq!(
            decode_err::<u64>(&integer_exponent).to_string(),
            "failed to decode into u64: 1 issue\n  at $: expected u64, found number 100.0"
        );
        assert_eq!(
            from_value::<f64>(&integer_exponent).expect("decodes"),
            100.0
        );

        let fractional_exponent: serde_json::Value =
            serde_json::from_str("1.5e1").expect("valid JSON");
        assert_eq!(
            from_value::<f64>(&fractional_exponent).expect("decodes"),
            15.0
        );
    }

    #[test]
    fn integer_beyond_u64_range_is_reported_as_lossy_float() {
        let value: serde_json::Value =
            serde_json::from_str("123456789012345678901234567890").expect("valid JSON");
        assert_eq!(
            decode_err::<u64>(&value).to_string(),
            "failed to decode into u64: 1 issue\n  at $: expected u64, found number 1.2345678901234568e+29"
        );
    }

    #[test]
    fn bool_happy_and_mismatch() {
        assert!(from_value::<bool>(&json!(true)).expect("decodes"));
        assert!(!from_value::<bool>(&json!(false)).expect("decodes"));
        assert_eq!(
            decode_err::<bool>(&json!(1)).to_string(),
            "failed to decode into bool: 1 issue\n  at $: expected bool, found number 1"
        );
    }

    #[test]
    fn string_happy_and_no_number_coercion() {
        assert_eq!(from_value::<String>(&json!("hi")).expect("decodes"), "hi");
        assert_eq!(
            decode_err::<String>(&json!(12)).to_string(),
            "failed to decode into string: 1 issue\n  at $: expected string, found number 12"
        );
    }

    #[test]
    fn option_null_value_and_mismatch() {
        assert_eq!(
            from_value::<Option<u64>>(&json!(null)).expect("decodes"),
            None
        );
        assert_eq!(
            from_value::<Option<u64>>(&json!(5)).expect("decodes"),
            Some(5)
        );
        assert_eq!(
            decode_err::<Option<u64>>(&json!("5")).to_string(),
            "failed to decode into optional u64: 1 issue\n  at $: expected u64, found string \"5\""
        );
    }

    #[test]
    fn vec_accumulates_every_element_failure() {
        let err = decode_err::<Vec<u64>>(&json!(["1500", 2, null, 4, false]));
        assert_eq!(err.issues().len(), 3);
        assert_eq!(
            err.to_string(),
            "failed to decode into array of u64: 3 issues\n  at $[0]: expected u64, found string \"1500\"\n  at $[2]: expected u64, found null\n  at $[4]: expected u64, found boolean false"
        );
    }

    #[test]
    fn vec_mismatch_on_non_array() {
        assert_eq!(
            decode_err::<Vec<u64>>(&json!(1)).to_string(),
            "failed to decode into array of u64: 1 issue\n  at $: expected array of u64, found number 1"
        );
    }

    #[test]
    fn maps_accumulate_every_value_failure() {
        let value = json!({"a": 1, "b": "no", "c": null});
        let err = decode_err::<BTreeMap<String, u64>>(&value);
        assert_eq!(err.issues().len(), 2);
        assert_eq!(
            err.to_string(),
            "failed to decode into object of u64: 2 issues\n  at $.b: expected u64, found string \"no\"\n  at $.c: expected u64, found null"
        );

        let err = decode_err::<HashMap<String, u64>>(&value);
        assert_eq!(err.issues().len(), 2);

        let ok: BTreeMap<String, u64> = from_value(&json!({"a": 1, "b": 2})).expect("decodes");
        assert_eq!(ok.len(), 2);
        let ok: HashMap<String, u64> = from_value(&json!({"a": 1})).expect("decodes");
        assert_eq!(ok.len(), 1);
    }

    #[test]
    fn duplicate_object_keys_keep_last_value_from_str() {
        let decoded =
            crate::from_str::<HashMap<String, u64>>(r#"{"a": 1, "a": 2}"#).expect("decodes");
        assert_eq!(decoded, HashMap::from([(String::from("a"), 2)]));
    }

    #[test]
    fn duplicate_object_keys_keep_last_value_from_value() {
        let value: serde_json::Value =
            serde_json::from_str(r#"{"a": 1, "a": 2}"#).expect("valid JSON");
        let decoded = from_value::<HashMap<String, u64>>(&value).expect("decodes");
        assert_eq!(decoded, HashMap::from([(String::from("a"), 2)]));
    }

    #[test]
    fn empty_containers_decode() {
        assert_eq!(
            from_value::<Vec<u64>>(&json!([])).expect("decodes"),
            Vec::<u64>::new()
        );
        assert_eq!(
            from_value::<HashMap<String, u64>>(&json!({})).expect("decodes"),
            HashMap::new()
        );
    }

    #[test]
    fn hash_map_issues_follow_key_sorted_iteration_order() {
        let err = decode_err::<HashMap<String, u64>>(&json!({"z": "x", "a": "y"}));
        assert_eq!(
            err.to_string(),
            "failed to decode into object of u64: 2 issues\n  at $.a: expected u64, found string \"y\"\n  at $.z: expected u64, found string \"x\""
        );
    }

    #[test]
    fn map_mismatch_on_non_object() {
        assert_eq!(
            decode_err::<BTreeMap<String, bool>>(&json!([])).to_string(),
            "failed to decode into object of bool: 1 issue\n  at $: expected object of bool, found array (len 0)"
        );
    }

    #[test]
    fn nested_paths_render_every_segment() {
        type Nested = BTreeMap<String, BTreeMap<String, Vec<BTreeMap<String, u64>>>>;
        let value = json!({"a": {"b": [{"c": 1}, {"c": 2}, {"c": "bad"}]}});
        let err = decode_err::<Nested>(&value);
        assert_eq!(err.issues().len(), 1);
        assert_eq!(err.issues()[0].path.to_string(), "$.a.b[2].c");
    }

    #[test]
    fn non_identifier_map_keys_are_quoted_in_paths() {
        let err = decode_err::<BTreeMap<String, u64>>(&json!({"weird key": null}));
        assert_eq!(
            err.to_string(),
            "failed to decode into object of u64: 1 issue\n  at $[\"weird key\"]: expected u64, found null"
        );
    }

    #[test]
    fn unicode_map_keys_are_quoted_in_paths() {
        let err = decode_err::<HashMap<String, u64>>(&json!({"ключ": null}));
        assert_eq!(
            err.to_string(),
            "failed to decode into object of u64: 1 issue\n  at $[\"ключ\"]: expected u64, found null"
        );
    }

    #[test]
    fn long_found_strings_are_truncated() {
        let long = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let err = decode_err::<u64>(&json!(long));
        assert_eq!(
            err.to_string(),
            "failed to decode into u64: 1 issue\n  at $: expected u64, found string \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa…\""
        );
    }

    #[test]
    fn found_object_descriptions() {
        assert_eq!(
            decode_err::<u64>(&json!({})).to_string(),
            "failed to decode into u64: 1 issue\n  at $: expected u64, found object (empty)"
        );
        let big = json!({"k1": 1, "k2": 2, "k3": 3, "k4": 4, "k5": 5, "k6": 6});
        assert_eq!(
            decode_err::<u64>(&big).to_string(),
            "failed to decode into u64: 1 issue\n  at $: expected u64, found object with keys \"k1\", \"k2\", \"k3\", \"k4\", \"k5\", … (6 total)"
        );
    }

    #[test]
    fn box_delegates() {
        assert_eq!(Box::<u64>::expected(), "u64");
        assert_eq!(*from_value::<Box<u64>>(&json!(7)).expect("decodes"), 7);
        assert_eq!(
            decode_err::<Box<u64>>(&json!(null)).to_string(),
            "failed to decode into u64: 1 issue\n  at $: expected u64, found null"
        );
    }

    #[test]
    fn value_accepts_anything() {
        let value = json!({"anything": [1, "two", null]});
        assert_eq!(
            from_value::<serde_json::Value>(&value).expect("decodes"),
            value
        );
        assert_eq!(serde_json::Value::expected(), "any JSON value");
    }

    #[test]
    fn mismatch_issue_exposes_expected_and_found() {
        let err = decode_err::<Vec<u64>>(&json!(["x"]));
        match &err.issues()[0].kind {
            IssueKind::Mismatch { expected, found } => {
                assert_eq!(expected.as_ref(), "u64");
                assert_eq!(found, "string \"x\"");
            }
            other => panic!("unexpected kind: {other:?}"),
        }
    }

    #[test]
    fn expected_strings_compose() {
        assert_eq!(Option::<String>::expected(), "optional string");
        assert_eq!(Vec::<u64>::expected(), "array of u64");
        assert_eq!(BTreeMap::<String, bool>::expected(), "object of bool");
        assert_eq!(HashMap::<String, f64>::expected(), "object of f64");
        assert_eq!(
            Vec::<Option<Vec<u8>>>::expected(),
            "array of optional array of u8"
        );
    }
}
