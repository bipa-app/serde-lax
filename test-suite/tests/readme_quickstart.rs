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

#[test]
fn quick_start_error_output_matches_the_readme() {
    let decoded = serde_lax::from_str::<Invoice>(
        r#"{"id":1,"amount":1500,"lineItems":[{"sku":"widget","quantity":2}],"status":"Pending"}"#,
    )
    .expect("decodes");
    assert_eq!(decoded.id, 1);
    assert_eq!(decoded.amount, 1500);
    assert_eq!(decoded.description, None);
    assert_eq!(decoded.line_items.len(), 1);
    assert_eq!(decoded.line_items[0].sku, "widget");
    assert_eq!(decoded.line_items[0].quantity, 2);
    match decoded.status {
        Status::Pending => {}
        Status::Paid => panic!("must decode Pending"),
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
}
