use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Fields, Path};

use super::attributes::{field, BoundType};

/// function which generates the hidden `ENCODES_TO_ZERO_BYTES` constant of a struct impl:
/// it holds iff every field which is (de)serialized encodes to zero bytes.
///
/// - skipped fields are not encoded, so they do not count;
/// - a field with `serialize_with`/`deserialize_with` is opaque, so it is assumed to encode
///   at least one byte.
///
/// The constants of fields are read through a `const fn`, so that a recursive type
/// doesn't make the constant cyclic (see `borsh::__private::encodes_to_zero_bytes_de`).
///
/// Field types are pasted with the user's spans, so lints that the user silenced on the struct
/// or field (such as `deprecated`) would fire again inside the constant; they're allowed on it.
pub fn encodes_to_zero_bytes_const(
    fields: &Fields,
    cratename: &Path,
    bound_type: BoundType,
) -> syn::Result<TokenStream2> {
    let mut terms = vec![];
    for field in fields {
        let parsed = field::Attributes::parse(&field.attrs)?;
        if parsed.skip {
            continue;
        }
        let (custom_with, read_const) = match bound_type {
            BoundType::Serialize => (
                parsed.serialize_with.is_some(),
                quote! { #cratename::__private::encodes_to_zero_bytes_ser },
            ),
            BoundType::Deserialize => (
                parsed.deserialize_with.is_some(),
                quote! { #cratename::__private::encodes_to_zero_bytes_de },
            ),
        };
        if custom_with {
            return Ok(zero_bytes_const(quote! { false }));
        }
        let field_type = &field.ty;
        terms.push(quote! { #read_const::<#field_type>() });
    }
    if terms.is_empty() {
        return Ok(zero_bytes_const(quote! { true }));
    }
    Ok(zero_bytes_const(quote! { #(#terms)&&* }))
}

fn zero_bytes_const(value: TokenStream2) -> TokenStream2 {
    quote! {
        #[allow(deprecated, unused_parens, unused_qualifications, clippy::all, clippy::use_self)]
        const ENCODES_TO_ZERO_BYTES: bool = #value;
    }
}
