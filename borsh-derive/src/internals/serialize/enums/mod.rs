use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Fields, Ident, ItemEnum, Lifetime, Path, Token, Variant};

use crate::internals::{
    attributes::{field, item, BoundType},
    enum_discriminant::Discriminants,
    generics, serialize,
};

pub fn process<const IS_ASYNC: bool>(
    input: ItemEnum,
    cratename: Path,
) -> syn::Result<TokenStream2> {
    let enum_ident = &input.ident;
    let use_discriminant = item::contains_use_discriminant(&input)?;
    let generics = generics::without_defaults(input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut where_clause = generics::default_where(where_clause);
    let mut generics_output = serialize::GenericsOutput::new(&generics);
    let mut all_variants_idx_body = TokenStream2::new();
    let mut fields_body = TokenStream2::new();
    let discriminants = Discriminants::new(&input.variants);
    let mut has_unit_variant = false;

    for (variant_idx, variant) in input.variants.iter().enumerate() {
        let variant_ident = &variant.ident;
        let discriminant_value = discriminants.get(variant_ident, use_discriminant, variant_idx)?;
        let variant_output = process_variant::<IS_ASYNC>(
            variant,
            enum_ident,
            &discriminant_value,
            &cratename,
            &mut generics_output,
        )?;
        all_variants_idx_body.extend(variant_output.variant_idx_body);
        match variant_output.body {
            VariantBody::Unit => has_unit_variant = true,
            VariantBody::Fields(VariantFields { header, body }) => fields_body.extend(quote!(
                #enum_ident::#variant_ident #header => {
                    #body
                }
            )),
        }
    }
    let fields_body = optimize_fields_body(fields_body, has_unit_variant);
    generics_output.extend::<IS_ASYNC>(&mut where_clause, &cratename);

    let serialize_trait = Ident::new(
        if IS_ASYNC {
            "BorshSerializeAsync"
        } else {
            "BorshSerialize"
        },
        Span::call_site(),
    );
    let writer_trait = if IS_ASYNC {
        quote! { async_io::AsyncWrite }
    } else {
        quote! { io::Write }
    };
    let r#async = IS_ASYNC.then(|| Token![async](Span::call_site()));
    let lifetime = IS_ASYNC.then(|| Lifetime::new("'async_variant", Span::call_site()));
    let lt_comma = IS_ASYNC.then(|| Token![,](Span::call_site()));

    let write_variant_idx = if IS_ASYNC {
        quote! { writer.write_u8(variant_idx).await }
    } else {
        quote! { writer.write_all(&variant_idx.to_le_bytes()) }
    };

    Ok(quote! {
        impl #impl_generics #cratename::ser::#serialize_trait for #enum_ident #ty_generics #where_clause {
            #r#async fn serialize<#lifetime #lt_comma __W: #cratename::#writer_trait>(
                &#lifetime self,
                writer: &#lifetime mut __W,
            ) -> ::core::result::Result<(), #cratename::io::Error> {
                let variant_idx: u8 = match self {
                    #all_variants_idx_body
                };
                #write_variant_idx?;

                #fields_body
                ::core::result::Result::Ok(())
            }
        }
    })
}

fn optimize_fields_body(fields_body: TokenStream2, has_unit_variant: bool) -> TokenStream2 {
    if fields_body.is_empty() {
        // If we no variants with fields, there's nothing to match against. Just
        // re-use the empty token stream.
        fields_body
    } else {
        let unit_fields_catchall = if has_unit_variant {
            // We had some variants with unit fields, create a catch-all for
            // these to be used at the bottom.
            quote!(
                _ => {}
            )
        } else {
            TokenStream2::new()
        };
        // Create a match that serialises all the fields for each non-unit
        // variant and add a catch-all at the bottom if we do have unit
        // variants.
        quote!(
            match self {
                #fields_body
                #unit_fields_catchall
            }
        )
    }
}

#[derive(Default)]
struct VariantFields {
    header: TokenStream2,
    body: TokenStream2,
}

impl VariantFields {
    fn named_header(self) -> Self {
        let header = self.header;

        VariantFields {
            // `..` pattern matching works even if all fields were specified
            header: quote! { { #header.. }},
            body: self.body,
        }
    }
    fn unnamed_header(self) -> Self {
        let header = self.header;

        VariantFields {
            header: quote! { ( #header )},
            body: self.body,
        }
    }
}

enum VariantBody {
    // No body variant, unit enum variant.
    Unit,
    // Variant with body (fields)
    Fields(VariantFields),
}

struct VariantOutput {
    body: VariantBody,
    variant_idx_body: TokenStream2,
}

fn process_variant<const IS_ASYNC: bool>(
    variant: &Variant,
    enum_ident: &Ident,
    discriminant_value: &TokenStream2,
    cratename: &Path,
    generics: &mut serialize::GenericsOutput,
) -> syn::Result<VariantOutput> {
    let variant_ident = &variant.ident;
    let variant_output = match &variant.fields {
        Fields::Named(fields) => {
            let mut variant_fields = VariantFields::default();
            for field in &fields.named {
                let field_id = serialize::FieldId::Enum(field.ident.clone().unwrap());
                process_field::<IS_ASYNC>(
                    field,
                    field_id,
                    cratename,
                    generics,
                    &mut variant_fields,
                )?;
            }
            VariantOutput {
                body: VariantBody::Fields(variant_fields.named_header()),
                variant_idx_body: quote!(
                    #enum_ident::#variant_ident {..} => #discriminant_value,
                ),
            }
        }
        Fields::Unnamed(fields) => {
            let mut variant_fields = VariantFields::default();
            for (field_idx, field) in fields.unnamed.iter().enumerate() {
                let field_id = serialize::FieldId::new_enum_unnamed(field_idx)?;
                process_field::<IS_ASYNC>(
                    field,
                    field_id,
                    cratename,
                    generics,
                    &mut variant_fields,
                )?;
            }
            VariantOutput {
                body: VariantBody::Fields(variant_fields.unnamed_header()),
                variant_idx_body: quote!(
                    #enum_ident::#variant_ident(..) => #discriminant_value,
                ),
            }
        }
        Fields::Unit => VariantOutput {
            body: VariantBody::Unit,
            variant_idx_body: quote!(
                #enum_ident::#variant_ident => #discriminant_value,
            ),
        },
    };
    Ok(variant_output)
}

fn process_field<const IS_ASYNC: bool>(
    field: &syn::Field,
    field_id: serialize::FieldId,
    cratename: &Path,
    generics: &mut serialize::GenericsOutput,
    output: &mut VariantFields,
) -> syn::Result<()> {
    let parsed = field::Attributes::parse(&field.attrs)?;

    let needs_bounds_derive = parsed.needs_bounds_derive::<IS_ASYNC>(BoundType::Serialize);
    generics
        .overrides
        .extend(parsed.collect_bounds::<IS_ASYNC>(BoundType::Serialize));

    let field_variant_header = field_id.enum_variant_header(parsed.skip);
    if let Some(field_variant_header) = field_variant_header {
        output.header.extend(field_variant_header);
    }

    if !parsed.skip {
        let delta = field_id.serialize_output::<IS_ASYNC>(
            &field.ty,
            cratename,
            if IS_ASYNC {
                parsed.serialize_with_async
            } else {
                parsed.serialize_with
            },
        );
        output.body.extend(delta);
        if needs_bounds_derive {
            generics.serialize_visitor.visit_field(field);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;
    use crate::internals::test_helpers::{
        default_cratename, local_insta_assert_snapshot, pretty_print_syn_str,
    };
    #[test]
    fn borsh_skip_tuple_variant_field() {
        let item_enum: ItemEnum = parse_quote! {
            enum AATTB {
                B(#[borsh(skip)] i32, #[borsh(skip)] u32),

                NegatedVariant {
                    beta: u8,
                }
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn struct_variant_field() {
        let item_enum: ItemEnum = parse_quote! {
            enum AB {
                B {
                    c: i32,
                    d: u32,
                },

                NegatedVariant {
                    beta: String,
                }
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn simple_enum_with_custom_crate() {
        let item_enum: ItemEnum = parse_quote! {
            enum AB {
                B {
                    c: i32,
                    d: u32,
                },

                NegatedVariant {
                    beta: String,
                }
            }
        };

        let crate_: Path = parse_quote! { reexporter::borsh };

        let actual = process::<false>(item_enum.clone(), crate_.clone()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, crate_).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn borsh_skip_struct_variant_field() {
        let item_enum: ItemEnum = parse_quote! {

            enum AB {
                B {
                    #[borsh(skip)]
                    c: i32,

                    d: u32,
                },

                NegatedVariant {
                    beta: String,
                }
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn borsh_skip_struct_variant_all_fields() {
        let item_enum: ItemEnum = parse_quote! {

            enum AAB {
                B {
                    #[borsh(skip)]
                    c: i32,

                    #[borsh(skip)]
                    d: u32,
                },

                NegatedVariant {
                    beta: String,
                }
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn simple_generics() {
        let item_enum: ItemEnum = parse_quote! {
            enum A<K, V, U> {
                B {
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, Vec<U>),
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn bound_generics() {
        let item_enum: ItemEnum = parse_quote! {
            enum A<K: Key, V, U> where V: Value {
                B {
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, Vec<U>),
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn recursive_enum() {
        let item_enum: ItemEnum = parse_quote! {
            enum A<K: Key, V> where V: Value {
                B {
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, Vec<A>),
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn generic_borsh_skip_struct_field() {
        let item_enum: ItemEnum = parse_quote! {
            enum A<K: Key, V, U> where V: Value {
                B {
                    #[borsh(skip)]
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, Vec<U>),
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn generic_borsh_skip_tuple_field() {
        let item_enum: ItemEnum = parse_quote! {
            enum A<K: Key, V, U> where V: Value {
                B {
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, #[borsh(skip)] Vec<U>),
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn generic_serialize_bound() {
        let item_enum: ItemEnum = parse_quote! {
            enum A<T: Debug, U> {
                C {
                    a: String,
                    #[borsh(bound(serialize =
                        "T: borsh::ser::BorshSerialize + PartialOrd,
                         U: borsh::ser::BorshSerialize"
                    ))]
                    b: HashMap<T, U>,
                },
                D(u32, u32),
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn generic_serialize_async_bound() {
        let item_enum: ItemEnum = parse_quote! {
            enum A<T: Debug, U> {
                C {
                    a: String,
                    #[borsh(async_bound(serialize =
                        "T: borsh::ser::BorshSerializeAsync + PartialOrd,
                         U: borsh::ser::BorshSerializeAsync"
                    ))]
                    b: HashMap<T, U>,
                },
                D(u32, u32),
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn check_serialize_with_attr() {
        let item_enum: ItemEnum = parse_quote! {
            enum C<K: Ord, V> {
                C3(u64, u64),
                C4 {
                    x: u64,
                    #[borsh(serialize_with = "third_party_impl::serialize_third_party")]
                    y: ThirdParty<K, V>
                },
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn check_serialize_with_async_attr() {
        let item_enum: ItemEnum = parse_quote! {
            enum C<K: Ord, V> {
                C3(u64, u64),
                C4 {
                    x: u64,
                    #[borsh(serialize_with_async = "third_party_impl::serialize_third_party")]
                    y: ThirdParty<K, V>
                },
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn borsh_discriminant_false() {
        let item_enum: ItemEnum = parse_quote! {
           #[borsh(use_discriminant = false)]
            enum X {
                A,
                B = 20,
                C,
                D,
                E = 10,
                F,
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }
    #[test]
    fn borsh_discriminant_true() {
        let item_enum: ItemEnum = parse_quote! {
            #[borsh(use_discriminant = true)]
            enum X {
                A,
                B = 20,
                C,
                D,
                E = 10,
                F,
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn mixed_with_unit_variants() {
        let item_enum: ItemEnum = parse_quote! {
            enum X {
                A(u16),
                B,
                C {x: i32, y: i32},
                D,
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }
}
