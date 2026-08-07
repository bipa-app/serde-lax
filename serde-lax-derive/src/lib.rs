//! Derive macro for `serde-lax`.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, Attribute, Data, DataEnum, DataStruct, DeriveInput, Fields, Ident, LitStr,
    Path, Type,
};

const SUPPORTED_SHAPES: &str = "serde-lax v0.1 supports only non-generic structs with named fields and enums whose variants are all unit variants";
const CONTAINER_ATTRIBUTES: &str =
    "unsupported #[lax] container attribute; supported: rename_all, no_serde";
const FIELD_ATTRIBUTES: &str =
    "unsupported #[lax] field attribute; supported: rename, default, with_serde";
const VARIANT_ATTRIBUTES: &str = "unsupported #[lax] enum variant attribute; supported: rename";
const RENAME_ALL_VALUES: &str = "rename_all must be one of: camelCase, snake_case, SCREAMING_SNAKE_CASE, kebab-case, PascalCase, lowercase, UPPERCASE";

/// Derives lax JSON decoding for a named-field struct or a unit-only enum.
///
/// The derive emits a `serde_lax::FromJson` implementation that visits every
/// field before returning an error. By default it also emits a
/// `serde::Deserialize` implementation that routes ordinary Serde JSON
/// decoding through `serde-lax`, preserving the collected multi-issue message.
/// Add `#[lax(no_serde)]` to emit only `FromJson`:
///
/// ```ignore
/// #[derive(serde_lax::Deserialize)]
/// #[lax(no_serde)]
/// struct Config { enabled: bool }
/// ```
///
/// Container `rename_all` changes struct field keys or unit enum strings. It
/// accepts `camelCase`, `snake_case`, `SCREAMING_SNAKE_CASE`, `kebab-case`,
/// `PascalCase`, `lowercase`, and `UPPERCASE`:
///
/// ```ignore
/// #[derive(serde_lax::Deserialize)]
/// #[lax(rename_all = "camelCase")]
/// struct User { display_name: String }
/// ```
///
/// A field or enum variant can override that rule with `rename`:
///
/// ```ignore
/// #[derive(serde_lax::Deserialize)]
/// enum State { #[lax(rename = "in-progress")] InProgress }
/// ```
///
/// Bare `default` uses [`Default::default`] when a key is missing, while
/// `default = "path::to::function"` calls the named zero-argument function:
///
/// ```ignore
/// fn default_limit() -> u64 { 100 }
///
/// #[derive(serde_lax::Deserialize)]
/// struct Page {
///     #[lax(default)] offset: u64,
///     #[lax(default = "default_limit")] limit: u64,
/// }
/// ```
///
/// `with_serde` delegates a field to `serde_json::from_value`. This supports
/// foreign types that implement Serde's `Deserialize` but not `FromJson`:
///
/// ```ignore
/// #[derive(serde_lax::Deserialize)]
/// struct Server { #[lax(with_serde)] address: std::net::IpAddr }
/// ```
///
/// A syntactically visible `Option<T>` may be absent without an issue. Type
/// aliases to `Option<T>` cannot be detected and remain required in v0.1.
///
/// Generic types, tuple/newtype/unit structs, enums with data, unions, unknown
/// attributes, and invalid attribute values produce a one-line compile error.
#[proc_macro_derive(Deserialize, attributes(lax))]
pub fn derive_deserialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

fn expand(input: DeriveInput) -> syn::Result<TokenStream2> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(input.generics, SUPPORTED_SHAPES));
    }

    let options = parse_container_options(&input.attrs)?;
    let ident = input.ident;
    let from_json_impl = match input.data {
        Data::Struct(data) => expand_struct(&ident, data, options.rename_all)?,
        Data::Enum(data) => expand_enum(&ident, data, options.rename_all)?,
        Data::Union(data) => {
            return Err(syn::Error::new_spanned(data.union_token, SUPPORTED_SHAPES))
        }
    };
    let serde_impl = if options.no_serde {
        TokenStream2::new()
    } else {
        expand_serde_impl(&ident)
    };

    Ok(quote! {
        #from_json_impl
        #serde_impl
    })
}

#[derive(Default)]
struct ContainerOptions {
    rename_all: Option<RenameAll>,
    no_serde: bool,
}

fn parse_container_options(attributes: &[Attribute]) -> syn::Result<ContainerOptions> {
    let mut options = ContainerOptions::default();
    for attribute in lax_attributes(attributes) {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename_all") {
                if options.rename_all.is_some() {
                    return Err(meta.error("duplicate rename_all attribute"));
                }
                let value: LitStr = meta.value()?.parse()?;
                options.rename_all = Some(RenameAll::parse(&value)?);
                return Ok(());
            }
            if meta.path.is_ident("no_serde") {
                if options.no_serde {
                    return Err(meta.error("duplicate no_serde attribute"));
                }
                reject_attribute_value(&meta, "no_serde does not accept a value")?;
                options.no_serde = true;
                return Ok(());
            }
            Err(meta.error(CONTAINER_ATTRIBUTES))
        })?;
    }
    Ok(options)
}

#[derive(Clone, Copy)]
enum RenameAll {
    Camel,
    Snake,
    ScreamingSnake,
    Kebab,
    Pascal,
    Lower,
    Upper,
}

impl RenameAll {
    fn parse(value: &LitStr) -> syn::Result<Self> {
        match value.value().as_str() {
            "camelCase" => Ok(Self::Camel),
            "snake_case" => Ok(Self::Snake),
            "SCREAMING_SNAKE_CASE" => Ok(Self::ScreamingSnake),
            "kebab-case" => Ok(Self::Kebab),
            "PascalCase" => Ok(Self::Pascal),
            "lowercase" => Ok(Self::Lower),
            "UPPERCASE" => Ok(Self::Upper),
            _ => Err(syn::Error::new(value.span(), RENAME_ALL_VALUES)),
        }
    }

    fn apply(self, name: &str) -> String {
        let words = split_words(name);
        match self {
            Self::Camel => camel_case(&words),
            Self::Snake => words.join("_"),
            Self::ScreamingSnake => words.join("_").to_uppercase(),
            Self::Kebab => words.join("-"),
            Self::Pascal => words.iter().map(|word| capitalize(word)).collect(),
            Self::Lower => words.concat(),
            Self::Upper => words.concat().to_uppercase(),
        }
    }
}

fn split_words(name: &str) -> Vec<String> {
    let name = match name.strip_prefix("r#") {
        Some(stripped) => stripped,
        None => name,
    };
    let characters = name.chars().collect::<Vec<_>>();
    let mut words = Vec::new();
    let mut current = String::new();

    for (index, character) in characters.iter().copied().enumerate() {
        if character == '_' || character == '-' || character.is_whitespace() {
            push_word(&mut words, &mut current);
            continue;
        }

        let previous = index
            .checked_sub(1)
            .and_then(|i| characters.get(i))
            .copied();
        let next = characters.get(index + 1).copied();
        let starts_word = character.is_uppercase()
            && (previous.is_some_and(char::is_lowercase)
                || previous.is_some_and(char::is_numeric)
                || (previous.is_some_and(char::is_uppercase)
                    && next.is_some_and(char::is_lowercase)));
        if starts_word {
            push_word(&mut words, &mut current);
        }
        current.extend(character.to_lowercase());
    }
    push_word(&mut words, &mut current);
    words
}

fn push_word(words: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        words.push(std::mem::take(current));
    }
}

fn camel_case(words: &[String]) -> String {
    let Some((first, rest)) = words.split_first() else {
        return String::new();
    };
    let mut output = first.clone();
    for word in rest {
        output.push_str(&capitalize(word));
    }
    output
}

fn capitalize(word: &str) -> String {
    let mut characters = word.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };
    first.to_uppercase().chain(characters).collect()
}

fn expand_struct(
    ident: &Ident,
    data: DataStruct,
    rename_all: Option<RenameAll>,
) -> syn::Result<TokenStream2> {
    let Fields::Named(fields) = data.fields else {
        return Err(syn::Error::new_spanned(ident, SUPPORTED_SHAPES));
    };

    let mut decoders = Vec::new();
    let mut initializers = Vec::new();
    for (index, field) in fields.named.into_iter().enumerate() {
        let field_ident = match field.ident {
            Some(field_ident) => field_ident,
            None => return Err(syn::Error::new_spanned(ident, SUPPORTED_SHAPES)),
        };
        let options = parse_field_options(&field.attrs)?;
        let rust_name = field_ident.to_string();
        let key = effective_name(&rust_name, options.rename.as_ref(), rename_all);
        let key = LitStr::new(&key, field_ident.span());
        let holder = format_ident!("__serde_lax_field_{index}");
        let ty = field.ty;
        let present = present_field_decoder(&ty, options.with_serde);
        let missing = missing_field_decoder(&ty, &key, &options);

        decoders.push(quote! {
            let #holder = match __serde_lax_object.get(#key) {
                Some(__serde_lax_value) => __serde_lax_cx.with_key(#key, |__serde_lax_cx| {
                    #present
                }),
                None => #missing,
            };
        });
        initializers.push(quote! { #field_ident: #holder? });
    }

    let expected = LitStr::new(&format!("object `{ident}`"), ident.span());
    Ok(quote! {
        impl ::serde_lax::FromJson for #ident {
            fn expected() -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Borrowed(#expected)
            }

            fn from_json(
                __serde_lax_value: &::serde_lax::__private::serde_json::Value,
                __serde_lax_cx: &mut ::serde_lax::Context,
            ) -> ::std::option::Option<Self> {
                let Some(__serde_lax_object) = __serde_lax_value.as_object() else {
                    __serde_lax_cx.mismatch(
                        <Self as ::serde_lax::FromJson>::expected(),
                        __serde_lax_value,
                    );
                    return None;
                };
                let __serde_lax_before = __serde_lax_cx.issue_count();
                #(#decoders)*
                if __serde_lax_cx.issue_count() > __serde_lax_before {
                    return None;
                }
                Some(Self {
                    #(#initializers),*
                })
            }
        }
    })
}

#[derive(Default)]
struct FieldOptions {
    rename: Option<LitStr>,
    default: Option<FieldDefault>,
    with_serde: bool,
}

enum FieldDefault {
    Trait,
    Function(Path),
}

fn parse_field_options(attributes: &[Attribute]) -> syn::Result<FieldOptions> {
    let mut options = FieldOptions::default();
    for attribute in lax_attributes(attributes) {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                if options.rename.is_some() {
                    return Err(meta.error("duplicate rename attribute"));
                }
                options.rename = Some(meta.value()?.parse()?);
                return Ok(());
            }
            if meta.path.is_ident("default") {
                if options.default.is_some() {
                    return Err(meta.error("duplicate default attribute"));
                }
                if meta.input.peek(syn::Token![=]) {
                    let value: LitStr = meta.value()?.parse()?;
                    options.default = Some(FieldDefault::Function(value.parse()?));
                } else {
                    reject_attribute_value(&meta, "default accepts no value or a string path")?;
                    options.default = Some(FieldDefault::Trait);
                }
                return Ok(());
            }
            if meta.path.is_ident("with_serde") {
                if options.with_serde {
                    return Err(meta.error("duplicate with_serde attribute"));
                }
                reject_attribute_value(&meta, "with_serde does not accept a value")?;
                options.with_serde = true;
                return Ok(());
            }
            Err(meta.error(FIELD_ATTRIBUTES))
        })?;
    }
    Ok(options)
}

fn present_field_decoder(ty: &Type, with_serde: bool) -> TokenStream2 {
    if with_serde {
        quote! {
            match ::serde_lax::__private::serde_json::from_value::<#ty>(__serde_lax_value.clone()) {
                Ok(__serde_lax_decoded) => Some(__serde_lax_decoded),
                Err(__serde_lax_error) => {
                    __serde_lax_cx.custom(::std::format!(
                        "invalid {}: {}",
                        stringify!(#ty),
                        __serde_lax_error,
                    ));
                    None
                }
            }
        }
    } else {
        quote! {
            <#ty as ::serde_lax::FromJson>::from_json(__serde_lax_value, __serde_lax_cx)
        }
    }
}

fn missing_field_decoder(ty: &Type, key: &LitStr, options: &FieldOptions) -> TokenStream2 {
    if let Some(default) = &options.default {
        return match default {
            FieldDefault::Trait => quote! { Some(<#ty as ::std::default::Default>::default()) },
            FieldDefault::Function(function) => quote! { Some(#function()) },
        };
    }
    if is_option(ty) {
        return quote! { Some(None) };
    }

    let expected = if options.with_serde {
        quote! { stringify!(#ty) }
    } else {
        quote! { <#ty as ::serde_lax::FromJson>::expected() }
    };
    quote! {{
        __serde_lax_cx.missing_field(#key, #expected);
        None
    }}
}

fn is_option(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    if type_path.qself.is_some() {
        return false;
    }
    match type_path.path.segments.last() {
        Some(segment) => segment.ident == "Option",
        None => false,
    }
}

fn expand_enum(
    ident: &Ident,
    data: DataEnum,
    rename_all: Option<RenameAll>,
) -> syn::Result<TokenStream2> {
    let mut match_arms = Vec::new();
    let mut names = Vec::new();
    for variant in data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(variant, SUPPORTED_SHAPES));
        }
        let rename = parse_variant_rename(&variant.attrs)?;
        let name = effective_name(&variant.ident.to_string(), rename.as_ref(), rename_all);
        let name_literal = LitStr::new(&name, variant.ident.span());
        let variant_ident = variant.ident;
        match_arms.push(quote! { #name_literal => Some(Self::#variant_ident) });
        names.push(format!("{name:?}"));
    }

    let expected = LitStr::new(&format!("one of {}", names.join(" | ")), ident.span());
    Ok(quote! {
        impl ::serde_lax::FromJson for #ident {
            fn expected() -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Borrowed(#expected)
            }

            fn from_json(
                __serde_lax_value: &::serde_lax::__private::serde_json::Value,
                __serde_lax_cx: &mut ::serde_lax::Context,
            ) -> ::std::option::Option<Self> {
                match __serde_lax_value.as_str() {
                    Some(__serde_lax_string) => match __serde_lax_string {
                        #(#match_arms,)*
                        _ => {
                            __serde_lax_cx.mismatch(
                        <Self as ::serde_lax::FromJson>::expected(),
                        __serde_lax_value,
                    );
                            None
                        }
                    },
                    None => {
                        __serde_lax_cx.mismatch(
                        <Self as ::serde_lax::FromJson>::expected(),
                        __serde_lax_value,
                    );
                        None
                    }
                }
            }
        }
    })
}

fn parse_variant_rename(attributes: &[Attribute]) -> syn::Result<Option<LitStr>> {
    let mut rename = None;
    for attribute in lax_attributes(attributes) {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                if rename.is_some() {
                    return Err(meta.error("duplicate rename attribute"));
                }
                rename = Some(meta.value()?.parse()?);
                return Ok(());
            }
            Err(meta.error(VARIANT_ATTRIBUTES))
        })?;
    }
    Ok(rename)
}

fn effective_name(
    rust_name: &str,
    rename: Option<&LitStr>,
    rename_all: Option<RenameAll>,
) -> String {
    if let Some(rename) = rename {
        return rename.value();
    }
    match rename_all {
        Some(rule) => rule.apply(rust_name),
        None => match rust_name.strip_prefix("r#") {
            Some(stripped) => stripped.to_owned(),
            None => rust_name.to_owned(),
        },
    }
}

fn expand_serde_impl(ident: &Ident) -> TokenStream2 {
    quote! {
        impl<'de> ::serde_lax::__private::serde::Deserialize<'de> for #ident {
            fn deserialize<__SerdeLaxDeserializer>(
                __serde_lax_deserializer: __SerdeLaxDeserializer,
            ) -> ::std::result::Result<Self, __SerdeLaxDeserializer::Error>
            where
                __SerdeLaxDeserializer:
                    ::serde_lax::__private::serde::Deserializer<'de>,
            {
                let __serde_lax_value =
                    <::serde_lax::__private::serde_json::Value as
                        ::serde_lax::__private::serde::Deserialize<'de>>::deserialize(
                            __serde_lax_deserializer,
                        )?;
                match ::serde_lax::from_value::<Self>(&__serde_lax_value) {
                    Ok(__serde_lax_decoded) => Ok(__serde_lax_decoded),
                    Err(__serde_lax_error) => Err(
                        <__SerdeLaxDeserializer::Error as
                            ::serde_lax::__private::serde::de::Error>::custom(
                                __serde_lax_error,
                            ),
                    ),
                }
            }
        }
    }
}

fn lax_attributes(attributes: &[Attribute]) -> impl Iterator<Item = &Attribute> {
    attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("lax"))
}

fn reject_attribute_value(meta: &syn::meta::ParseNestedMeta<'_>, message: &str) -> syn::Result<()> {
    if meta.input.peek(syn::Token![=]) || meta.input.peek(syn::token::Paren) {
        Err(meta.error(message))
    } else {
        Ok(())
    }
}
