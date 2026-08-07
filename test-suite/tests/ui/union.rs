#[derive(serde_lax::Deserialize)]
union UnsupportedUnion {
    value: u64,
}

fn main() {}
