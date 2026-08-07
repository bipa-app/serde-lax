#[derive(serde_lax::Deserialize)]
#[lax(rename_all = "bogus")]
struct InvalidRenameAll {
    value: u64,
}

fn main() {}
