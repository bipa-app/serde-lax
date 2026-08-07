//! Derive macro for `serde-lax`.
//!
//! This crate is a placeholder: the real derive will be implemented later.
//! Until then, applying the derive produces a compile error at the use site.

use proc_macro::TokenStream;

/// Placeholder for `#[derive(serde_lax::Deserialize)]`.
///
/// A later unit replaces this stub with the real implementation. Using the
/// derive today produces a compile error at the use site.
#[proc_macro_derive(Deserialize, attributes(lax))]
pub fn derive_deserialize(_input: TokenStream) -> TokenStream {
    "compile_error!(\"#[derive(serde_lax::Deserialize)] is not implemented yet\");"
        .parse()
        .expect("stub token stream is valid Rust")
}
