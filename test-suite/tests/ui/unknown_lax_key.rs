#[derive(serde_lax::Deserialize)]
#[lax(bogus)]
struct UnknownLaxKey {
    value: u64,
}

fn main() {}
