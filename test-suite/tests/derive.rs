use std::net::{IpAddr, Ipv4Addr};

use serde_lax::Deserialize;

#[derive(Debug, PartialEq, Deserialize)]
struct Person {
    name: String,
    age: u64,
}

#[test]
fn struct_decodes_through_serde_lax() {
    let person: Person = serde_lax::from_str(r#"{"name":"Ada","age":37}"#).expect("decodes");
    assert_eq!(
        person,
        Person {
            name: "Ada".to_owned(),
            age: 37,
        }
    );
}

#[derive(Debug, PartialEq, Deserialize)]
struct Customer {
    name: String,
    tags: Vec<String>,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Order {
    count: u64,
    paid: bool,
    customer: Customer,
}

#[test]
fn struct_collects_all_nested_issues() {
    let input = r#"
        {
            "count": "many",
            "customer": {
                "name": 42,
                "tags": ["new", "priority", false]
            }
        }
    "#;
    let error = serde_lax::from_str::<Order>(input).expect_err("must fail");
    assert_eq!(
        error.to_string(),
        "failed to decode into object `Order`: 4 issues\n  at $.count: expected u64, found string \"many\"\n  at $.paid: missing required field (expected bool)\n  at $.customer.name: expected string, found number 42\n  at $.customer.tags[2]: expected string, found boolean false"
    );
}

#[derive(Debug, PartialEq, Deserialize)]
#[lax(rename_all = "camelCase")]
struct RenamedFields {
    display_name: String,
    #[lax(rename = "active")]
    is_active: bool,
}

#[test]
fn rename_all_and_field_rename_control_error_paths() {
    let error = serde_lax::from_str::<RenamedFields>(r#"{"displayName":false,"active":"yes"}"#)
        .expect_err("must fail");
    assert_eq!(
        error.to_string(),
        "failed to decode into object `RenamedFields`: 2 issues\n  at $.displayName: expected string, found boolean false\n  at $.active: expected bool, found string \"yes\""
    );
}

fn default_limit() -> u64 {
    25
}

#[derive(Debug, PartialEq, Deserialize)]
struct DefaultsAndOptions {
    #[lax(default)]
    enabled: bool,
    #[lax(default = "default_limit")]
    limit: u64,
    note: Option<String>,
    null_note: Option<String>,
    value_note: Option<String>,
}

#[test]
fn defaults_and_option_absence_are_supported() {
    let decoded: DefaultsAndOptions =
        serde_lax::from_str(r#"{"null_note":null,"value_note":"present"}"#).expect("decodes");
    assert_eq!(
        decoded,
        DefaultsAndOptions {
            enabled: false,
            limit: 25,
            note: None,
            null_note: None,
            value_note: Some("present".to_owned()),
        }
    );
}

#[derive(Debug, PartialEq, Deserialize)]
#[lax(rename_all = "kebab-case")]
enum Status {
    InProgress,
    #[lax(rename = "done")]
    Complete,
}

#[test]
fn unit_enum_decodes_effective_names_and_reports_expected_values() {
    assert_eq!(
        serde_lax::from_str::<Status>(r#""in-progress""#).expect("decodes"),
        Status::InProgress
    );
    assert_eq!(
        serde_lax::from_str::<Status>(r#""done""#).expect("decodes"),
        Status::Complete
    );

    let error = serde_lax::from_str::<Status>(r#""unknown""#).expect_err("must fail");
    assert_eq!(
        error.to_string(),
        "failed to decode into one of \"in-progress\" | \"done\": 1 issue\n  at $: expected one of \"in-progress\" | \"done\", found string \"unknown\""
    );
}

#[derive(Debug, PartialEq, Deserialize)]
struct NetworkConfig {
    #[lax(with_serde)]
    address: IpAddr,
}

#[test]
fn with_serde_decodes_foreign_types_and_records_custom_errors() {
    let decoded: NetworkConfig =
        serde_lax::from_str(r#"{"address":"127.0.0.1"}"#).expect("decodes");
    assert_eq!(
        decoded,
        NetworkConfig {
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
        }
    );

    let error =
        serde_lax::from_str::<NetworkConfig>(r#"{"address":"not-an-ip"}"#).expect_err("must fail");
    let rendered = error.to_string();
    assert!(
        rendered.starts_with(
            "failed to decode into object `NetworkConfig`: 1 issue\n  at $.address: invalid IpAddr:"
        ),
        "{rendered}"
    );
}

#[derive(Debug, PartialEq, Deserialize)]
struct DropIn {
    id: u64,
    label: String,
}

#[test]
fn serde_drop_in_matches_lax_decode_and_keeps_multi_issue_message() {
    let input = r#"{"id":7,"label":"seven"}"#;
    let lax: DropIn = serde_lax::from_str(input).expect("lax decode");
    let serde: DropIn = serde_json::from_str(input).expect("serde decode");
    assert_eq!(serde, lax);

    let error = serde_json::from_str::<DropIn>(r#"{"id":"seven"}"#).expect_err("must fail");
    let rendered = error.to_string();
    assert!(
        rendered.contains(
            "failed to decode into object `DropIn`: 2 issues\n  at $.id: expected u64, found string \"seven\"\n  at $.label: missing required field (expected string)"
        ),
        "{rendered}"
    );
}

#[derive(Debug, PartialEq, Deserialize)]
#[lax(no_serde)]
struct LaxOnly {
    value: u64,
}

#[test]
fn no_serde_still_emits_from_json() {
    let decoded: LaxOnly = serde_lax::from_str(r#"{"value":9}"#).expect("decodes");
    assert_eq!(decoded, LaxOnly { value: 9 });
}

#[derive(Debug, PartialEq, Deserialize)]
struct LineItem {
    price: u64,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Basket {
    items: Vec<LineItem>,
}

#[test]
fn nested_structs_inside_vecs_compose_paths() {
    let error = serde_lax::from_str::<Basket>(r#"{"items":[{"price":10},{"price":"unknown"}]}"#)
        .expect_err("must fail");
    assert_eq!(
        error.to_string(),
        "failed to decode into object `Basket`: 1 issue\n  at $.items[1].price: expected u64, found string \"unknown\""
    );
}
