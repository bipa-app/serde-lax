# serde-lax

Accept anything on the wire — report every mismatch in one pass.

[![CI](https://github.com/bipa-app/serde-lax/actions/workflows/ci.yml/badge.svg)](https://github.com/bipa-app/serde-lax/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/serde-lax.svg)](https://crates.io/crates/serde-lax)
[![docs.rs](https://docs.rs/serde-lax/badge.svg)](https://docs.rs/serde-lax)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Why

Real-world APIs are written in weakly typed languages and often don't match their own docs. When the JSON disagrees with your Rust types, serde stops at the first failure:

```text
invalid type: string "1500", expected u64 at line 2 column 20
```

serde-lax parses to `serde_json::Value` first — accept anything on the wire — then walks the value into your type and reports **every** mismatch in one pass: exact JSON path, expected type, and what was actually there:

```text
failed to decode into object `Invoice`: 3 issues
  at $.amount: expected u64, found string "1500"
  at $.customer.id: missing required field (expected string)
  at $.status: expected one of "pending" | "paid", found string "canceled"
```

## Quick start

```rust
#[derive(serde_lax::Deserialize)]
#[lax(rename_all = "camelCase")]
struct Invoice {
    id: u64,
    amount: u64,
    description: Option<String>,
    line_items: Vec<LineItem>,
    status: Status,
}

#[derive(serde_lax::Deserialize)]
struct LineItem {
    sku: String,
    quantity: u32,
}

#[derive(serde_lax::Deserialize)]
enum Status {
    Pending,
    Paid,
}

let json = r#"
{
  "id": 1,
  "amount": "1500",
  "lineItems": [{"sku": "widget", "quantity": 2}],
  "status": "canceled"
}
"#;
let error = match serde_lax::from_str::<Invoice>(json) {
    Ok(_) => panic!("must fail"),
    Err(error) => error,
};
assert_eq!(
    error.to_string(),
    "failed to decode into object `Invoice`: 2 issues\n  at $.amount: expected u64, found string \"1500\"\n  at $.status: expected one of \"Pending\" | \"Paid\", found string \"canceled\"",
);
```

Entry points, all returning `Result<T, serde_lax::Error>`:

- `serde_lax::from_str::<T>(s)`
- `serde_lax::from_slice::<T>(bytes)`
- `serde_lax::from_value::<T>(value)`

Use `Error::issues()` and `Error::is_syntax()` for programmatic access.

## Drop-in with serde / reqwest

The derive also emits a real `serde::Deserialize` impl, so existing `serde_json::from_str::<T>` and `reqwest::Response::json::<T>()` call sites pick up the rich message with zero changes. Adoption is swapping one attribute:

```rust
// before
#[derive(serde::Deserialize)]
// after
#[derive(serde_lax::Deserialize)]
```

(Use `#[lax(no_serde)]` on the container to skip emitting the serde impl, e.g. to avoid a conflicting manual impl.)

## Attributes

| Attribute | Where | Effect |
|---|---|---|
| `#[lax(rename_all = "camelCase")]` | container | rename all fields (any serde case convention) |
| `#[lax(no_serde)]` | container | skip emitting the `serde::Deserialize` impl |
| `#[lax(rename = "...")]` | field / variant | rename a single field or variant |
| `#[lax(default)]` | field | fall back to `Default::default()` when missing |
| `#[lax(default = "path::to::fn")]` | field | fall back to the given function when missing |
| `#[lax(with_serde)]` | field | decode a foreign type (e.g. `std::net::IpAddr`) via serde |

Supported shapes: structs with named fields, and enums where every variant is a unit variant.

## Semantics

- **Strict types, rich reporting.** No coercion — `"1500"` is not a `u64`. The laxness is in *how much* gets reported, not in what's accepted.
- `Option<T>` treats both missing and `null` as `None`.
- Unknown JSON fields are ignored.
- JSON-only by design.
- Rendered errors show at most the first 100 issues, with a `… and N more issues (not shown)` summary line; `Error::issues()` always contains all of them.
- Implementations provided for: integers, floats, `bool`, `String`, `Option`, `Vec`, `HashMap`/`BTreeMap`, `Box`, `serde_json::Value`.
- **Status:** 0.1.0 — early release. The API may change based on feedback.

## Comparison with alternatives

| Crate | What it does | Trade-off vs serde-lax |
|---|---|---|
| [serde_path_to_error](https://crates.io/crates/serde_path_to_error) (dtolnay) | Adds the field path to serde's **first** error via a wrapper deserializer | No multi-error; call sites must wrap. Best when you need a tiny format-neutral wrapper. |
| [eserde](https://crates.io/crates/eserde) (Mainmatter) | Collects several errors | Batch errors require switching call sites to `eserde::json::from_*`; two-pass on failure; its serde impl doesn't give batch errors through plain serde_json. |
| [deserr](https://crates.io/crates/deserr) (Meilisearch) | Own `Deserr<E>` trait with caller-defined error types; configurable accumulation | Not serde-compatible — aimed at validating inbound API requests with custom error codes. |
| [format_serde_error](https://crates.io/crates/format_serde_error) (dormant since 2021) | Pretty-prints serde's single error with source line + caret | Single error only. |
| **serde-lax** | Every mismatch in one pass + `$`-paths + the actual found value + drop-in via the emitted serde impl | JSON-only; buffers into `Value` (slower than serde's streaming happy path); strict scope — no custom error codes like deserr, no YAML/TOML. |

## MSRV

Rust 1.75.

## License

MIT. See [LICENSE](LICENSE).
