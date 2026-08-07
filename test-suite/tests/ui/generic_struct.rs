#[derive(serde_lax::Deserialize)]
struct GenericStruct<T> {
    value: T,
}

fn main() {}
