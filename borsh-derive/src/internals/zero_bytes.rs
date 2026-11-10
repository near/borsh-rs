use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{quote, quote_spanned};
use std::convert::TryFrom;
use syn::{spanned::Spanned, Fields, Ident, Index, Member, Path};

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
/// Field types are not pasted into the constant: the `const fn` infers each one from an
/// accessor of the field, `|s: &Self| addr_of!(s.field)`; a raw pointer, because a reference
/// to a field of a `#[repr(packed)]` struct doesn't compile. Pasted types would bring the user's tokens
/// into a new scope, where lints that the user silenced on the struct or field (such as
/// `deprecated`) fire again, and respanning them would change what names in const
/// expressions resolve to.
pub fn encodes_to_zero_bytes_const(
    fields: &Fields,
    cratename: &Path,
    bound_type: BoundType,
) -> syn::Result<TokenStream2> {
    // NOTE: a constant, const parameter or unit struct of the caller with the receiver's name
    // would turn the closure's parameter into a pattern; a local `const fn` of that name, called
    // so that it's not dead code, shadows them first
    let receiver = Ident::new("__borsh_struct", Span::mixed_site());
    let mut terms = vec![];
    for (index, field) in fields.iter().enumerate() {
        let parsed = field::Attributes::parse(&field.attrs)?;
        if parsed.skip {
            continue;
        }
        let (custom_with, read_const) = match bound_type {
            BoundType::Serialize => (
                parsed.serialize_with.is_some(),
                quote! { #cratename::__private::field_encodes_to_zero_bytes_ser },
            ),
            BoundType::Deserialize => (
                parsed.deserialize_with.is_some(),
                quote! { #cratename::__private::field_encodes_to_zero_bytes_de },
            ),
        };
        if custom_with {
            return Ok(zero_bytes_const(quote! { false }));
        }
        let member = match &field.ident {
            Some(ident) => Member::Named(ident.clone()),
            None => Member::Unnamed(Index {
                index: u32::try_from(index).map_err(|err| {
                    syn::Error::new(
                        Span::call_site(),
                        format!("up to 2^32 fields are supported {}", err),
                    )
                })?,
                span: Span::call_site(),
            }),
        };
        // located at the field's type, so that an unsatisfied bound points at it
        let span = Span::call_site().located_at(field.ty.span());
        terms.push(quote_spanned! {span=> #read_const(|#receiver: &Self| ::core::ptr::addr_of!(#receiver.#member)) });
    }
    if terms.is_empty() {
        return Ok(zero_bytes_const(quote! { true }));
    }
    Ok(zero_bytes_const(quote! {
        {
            const fn #receiver() {}
            #receiver();
            #(#terms)&&*
        }
    }))
}

fn zero_bytes_const(value: TokenStream2) -> TokenStream2 {
    quote! {
        const ENCODES_TO_ZERO_BYTES: bool = #value;
    }
}
