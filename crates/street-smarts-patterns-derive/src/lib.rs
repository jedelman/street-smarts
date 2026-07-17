//! `#[derive(Parameters)]` -- generates the same mechanical `impl Parameters`
//! every `P*Params` struct in `street-smarts-patterns` currently hand-writes:
//! `schema()`, `defaults()`, `as_vector()`, `from_vector()`, in field-declaration
//! order. See PATTERN_LANGUAGE_SIMULATION.md §3.3.
//!
//! Usage (matches the existing hand-written shape field-for-field):
//!
//! ```ignore
//! #[derive(Clone, serde::Serialize, serde::Deserialize, Parameters)]
//! pub struct P37Params {
//!     #[param(min = 2000.0, max = 20000.0, default = 7000.0, unit = "m²",
//!             desc = "Target land area per house-cluster block.")]
//!     pub target_block_area_m2: f64,
//!     #[param(min = 1.0, max = 10.0, default = 2.0, unit = "blocks", integer,
//!             desc = "Minimum block count regardless of area.")]
//!     pub min_blocks: f64,
//! }
//! ```
//!
//! Every field must carry a `#[param(...)]` attribute with `min`, `max`,
//! `default`, and `desc` (float literals / a string literal); `unit` and
//! `integer` are optional. This is deliberately not a generalized
//! "any struct" derive -- every field must be `f64`, matching every
//! existing `Params` struct in this codebase. A field of another type is a
//! compile error, not a silently-wrong generated impl.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Lit, Meta};

struct FieldSpec {
    ident: syn::Ident,
    min: f64,
    max: f64,
    default: f64,
    unit: Option<String>,
    desc: String,
    integer: bool,
}

#[proc_macro_derive(Parameters, attributes(param))]
pub fn derive_parameters(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    let Data::Struct(data) = &input.data else {
        return syn::Error::new_spanned(&input, "#[derive(Parameters)] only supports structs")
            .to_compile_error()
            .into();
    };
    let Fields::Named(fields) = &data.fields else {
        return syn::Error::new_spanned(&input, "#[derive(Parameters)] requires named fields")
            .to_compile_error()
            .into();
    };

    let mut specs = Vec::new();
    for field in &fields.named {
        let ident = field.ident.clone().expect("named field");
        match parse_param_attr(field) {
            Ok(spec) => specs.push(FieldSpec { ident, ..spec }),
            Err(e) => return e.to_compile_error().into(),
        }
    }

    let n = specs.len();
    let field_idents: Vec<_> = specs.iter().map(|s| &s.ident).collect();

    let schema_entries = specs.iter().map(|s| {
        let name_str = s.ident.to_string();
        let desc = &s.desc;
        let (min, max, default) = (s.min, s.max, s.default);
        let ctor = if s.integer {
            quote! { crate::parameters::ParamSpec::integer(#name_str, #desc, #min, #max, #default) }
        } else {
            quote! { crate::parameters::ParamSpec::float(#name_str, #desc, #min, #max, #default) }
        };
        match &s.unit {
            Some(u) => quote! { #ctor.with_unit(#u) },
            None => ctor,
        }
    });

    let default_fields = specs.iter().map(|s| {
        let ident = &s.ident;
        let default = s.default;
        quote! { #ident: #default }
    });

    let vector_fields = field_idents.iter().map(|ident| quote! { self.#ident });

    let from_vector_assigns = (0..n).map(|i| {
        let ident = &field_idents[i];
        quote! {
            if let (Some(s), Some(x)) = (schema.get(#i), v.get(#i)) {
                p.#ident = s.clamp(*x);
            }
        }
    });

    let expanded = quote! {
        impl crate::parameters::Parameters for #struct_name {
            fn schema() -> Vec<crate::parameters::ParamSpec> {
                vec![ #( #schema_entries ),* ]
            }
            fn defaults() -> Self {
                Self { #( #default_fields ),* }
            }
            fn as_vector(&self) -> Vec<f64> {
                vec![ #( #vector_fields ),* ]
            }
            fn from_vector(v: &[f64]) -> Self {
                let schema = <Self as crate::parameters::Parameters>::schema();
                let mut p = <Self as crate::parameters::Parameters>::defaults();
                #( #from_vector_assigns )*
                p
            }
        }
    };
    expanded.into()
}

fn parse_param_attr(field: &syn::Field) -> syn::Result<FieldSpec> {
    let attr = field
        .attrs
        .iter()
        .find(|a| a.path().is_ident("param"))
        .ok_or_else(|| syn::Error::new_spanned(field, "every field needs a #[param(...)] attribute"))?;

    let mut min = None;
    let mut max = None;
    let mut default = None;
    let mut unit = None;
    let mut desc = None;
    let mut integer = false;

    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("integer") {
            integer = true;
            return Ok(());
        }
        let value = meta.value()?;
        let lit: Lit = value.parse()?;
        if meta.path.is_ident("min") {
            min = Some(lit_to_f64(&lit)?);
        } else if meta.path.is_ident("max") {
            max = Some(lit_to_f64(&lit)?);
        } else if meta.path.is_ident("default") {
            default = Some(lit_to_f64(&lit)?);
        } else if meta.path.is_ident("unit") {
            unit = Some(lit_to_string(&lit)?);
        } else if meta.path.is_ident("desc") {
            desc = Some(lit_to_string(&lit)?);
        } else {
            return Err(meta.error("unknown #[param(...)] key -- expected min/max/default/unit/desc/integer"));
        }
        Ok(())
    })?;

    Ok(FieldSpec {
        ident: field.ident.clone().unwrap(),
        min: min.ok_or_else(|| syn::Error::new_spanned(attr, "#[param(...)] missing `min`"))?,
        max: max.ok_or_else(|| syn::Error::new_spanned(attr, "#[param(...)] missing `max`"))?,
        default: default.ok_or_else(|| syn::Error::new_spanned(attr, "#[param(...)] missing `default`"))?,
        desc: desc.ok_or_else(|| syn::Error::new_spanned(attr, "#[param(...)] missing `desc`"))?,
        unit,
        integer,
    })
}

fn lit_to_f64(lit: &Lit) -> syn::Result<f64> {
    match lit {
        Lit::Float(f) => f.base10_parse::<f64>(),
        Lit::Int(i) => i.base10_parse::<f64>(),
        _ => Err(syn::Error::new_spanned(lit, "expected a numeric literal")),
    }
}

fn lit_to_string(lit: &Lit) -> syn::Result<String> {
    match lit {
        Lit::Str(s) => Ok(s.value()),
        _ => Err(syn::Error::new_spanned(lit, "expected a string literal")),
    }
}

// Meta import kept for parse_nested_meta's closure argument type inference
// in older syn 2.x point releases; silence unused-import lint if unneeded.
#[allow(unused_imports)]
use syn::spanned::Spanned as _;
#[allow(dead_code)]
fn _unused(_m: &Meta) {}
