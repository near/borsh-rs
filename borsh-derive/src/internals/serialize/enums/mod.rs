use core::convert::TryFrom;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Fields, Ident, ItemEnum, Path, Variant, WhereClause, WherePredicate};

use crate::internals::{
    attributes::{field, item, BoundType},
    enum_discriminant, field_derive, generics,
};

pub fn process(input: &ItemEnum, cratename: Ident) -> syn::Result<TokenStream2> {
    let enum_ident = &input.ident;
    let generics = generics::without_defaults(&input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut where_clause = where_clause.map_or_else(
        || WhereClause {
            where_token: Default::default(),
            predicates: Default::default(),
        },
        Clone::clone,
    );

    let mut override_predicates: Vec<WherePredicate> = vec![];
    let mut serialize_params_visitor = generics::FindTyParams::new(&generics);

    let use_discriminant = item::contains_use_discriminant(input)?;

    let mut all_variants_idx_body = TokenStream2::new();
    let mut fields_body = TokenStream2::new();
    let discriminants = enum_discriminant::map(&input.variants);

    for (variant_idx, variant) in input.variants.iter().enumerate() {
        let variant_idx = u8::try_from(variant_idx).map_err(|err| {
            syn::Error::new(
                variant.ident.span(),
                format!("up to 256 enum variants are supported. error{}", err),
            )
        })?;
        let variant_ident = &variant.ident;
        let discriminant_value = discriminants.get(variant_ident).unwrap();
        let discriminant_value = if use_discriminant {
            quote! { #discriminant_value }
        } else {
            quote! { #variant_idx }
        };
        let variant_output = process_variant(
            variant,
            enum_ident,
            &discriminant_value,
            &cratename,
            &mut serialize_params_visitor,
            &mut override_predicates,
        )?;

        all_variants_idx_body.extend(variant_output.variant_idx_body);
        let variant_header = variant_output.header;
        let variant_body = variant_output.body;
        fields_body.extend(quote!(
            #enum_ident::#variant_ident #variant_header => {
                #variant_body
            }
        ))
    }
    let trait_path: Path = syn::parse2(quote! { #cratename::ser::BorshSerialize }).unwrap();
    let predicates =
        generics::compute_predicates(serialize_params_visitor.process_for_bounds(), &trait_path);
    where_clause.predicates.extend(predicates);
    where_clause.predicates.extend(override_predicates);
    Ok(quote! {
        impl #impl_generics #cratename::ser::BorshSerialize for #enum_ident #ty_generics #where_clause {
            fn serialize<W: #cratename::__private::maybestd::io::Write>(&self, writer: &mut W) -> ::core::result::Result<(), #cratename::__private::maybestd::io::Error> {
                let variant_idx: u8 = match self {
                    #all_variants_idx_body
                };
                writer.write_all(&variant_idx.to_le_bytes())?;

                match self {
                    #fields_body
                }
                Ok(())
            }
        }
    })
}

struct VariantOutput {
    header: TokenStream2,
    body: TokenStream2,
    variant_idx_body: TokenStream2,
}

impl VariantOutput {
    fn new() -> Self {
        Self {
            body: TokenStream2::new(),
            header: TokenStream2::new(),
            variant_idx_body: TokenStream2::new(),
        }
    }
}

fn process_variant(
    variant: &Variant,
    enum_ident: &Ident,
    discriminant_value: &TokenStream2,
    cratename: &Ident,
    serialize_params_visitor: &mut generics::FindTyParams,
    override_predicates: &mut Vec<WherePredicate>,
) -> syn::Result<VariantOutput> {
    let variant_ident = &variant.ident;
    let mut variant_output = VariantOutput::new();
    match &variant.fields {
        Fields::Named(fields) => {
            for field in &fields.named {
                let field_id =
                    field_derive::FieldID::EnumVariantNamed(field.ident.as_ref().unwrap().clone());
                process_field(
                    field,
                    field_id,
                    cratename,
                    serialize_params_visitor,
                    override_predicates,
                    &mut variant_output,
                )?;
            }
            let header = variant_output.header;
            // `..` pattern matching works even if all fields were specified
            variant_output.header = quote! { { #header.. }};
            variant_output.variant_idx_body = quote!(
                #enum_ident::#variant_ident {..} => #discriminant_value,
            );
        }
        Fields::Unnamed(fields) => {
            for (field_idx, field) in fields.unnamed.iter().enumerate() {
                let field_id = field_derive::FieldID::new_enum_index(field_idx)?;
                process_field(
                    field,
                    field_id,
                    cratename,
                    serialize_params_visitor,
                    override_predicates,
                    &mut variant_output,
                )?;
            }
            let header = variant_output.header;
            variant_output.header = quote! { ( #header )};
            variant_output.variant_idx_body = quote!(
                #enum_ident::#variant_ident(..) => #discriminant_value,
            );
        }
        Fields::Unit => {
            variant_output.variant_idx_body = quote!(
                #enum_ident::#variant_ident => #discriminant_value,
            );
        }
    };
    Ok(variant_output)
}

fn process_field(
    field: &syn::Field,
    field_id: field_derive::FieldID,
    cratename: &Ident,
    serialize_params_visitor: &mut generics::FindTyParams,
    override_predicates: &mut Vec<WherePredicate>,
    output: &mut VariantOutput,
) -> syn::Result<()> {
    let skipped = field::contains_skip(&field.attrs);
    let parsed = field::Attributes::parse(&field.attrs, skipped)?;

    let needs_bounds_derive = parsed.needs_bounds_derive(BoundType::Serialize);
    override_predicates.extend(parsed.collect_bounds(BoundType::Serialize));

    let field_variant_header = field_id.enum_variant_header(skipped);
    if let Some(field_variant_header) = field_variant_header {
        output.header.extend(field_variant_header);
    }

    if !skipped {
        let delta = field_id.serialize_output(cratename, parsed.serialize_with);
        output.body.extend(delta);
        if needs_bounds_derive {
            serialize_params_visitor.visit_field(field);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::internals::test_helpers::{local_insta_assert_snapshot, pretty_print_syn_str};

    use super::*;
    use proc_macro2::Span;
    #[test]
    fn borsh_skip_tuple_variant_field() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            enum AATTB {
                B(#[borsh_skip] i32, #[borsh_skip] u32),

                NegatedVariant {
                    beta: u8,
                }
            }
        })
        .unwrap();
        let actual = process(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn struct_variant_field() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            enum AB {
                B {
                    c: i32,
                    d: u32,
                },

                NegatedVariant {
                    beta: String,
                }
            }
        })
        .unwrap();

        let actual = process(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn borsh_skip_struct_variant_field() {
        let item_enum: ItemEnum = syn::parse2(quote! {

            enum AB {
                B {
                    #[borsh_skip]
                    c: i32,

                    d: u32,
                },

                NegatedVariant {
                    beta: String,
                }
            }
        })
        .unwrap();

        let actual = process(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn borsh_skip_struct_variant_all_fields() {
        let item_enum: ItemEnum = syn::parse2(quote! {

            enum AAB {
                B {
                    #[borsh_skip]
                    c: i32,

                    #[borsh_skip]
                    d: u32,
                },

                NegatedVariant {
                    beta: String,
                }
            }
        })
        .unwrap();

        let actual = process(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn simple_generics() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum A<K, V, U> {
                B {
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, Vec<U>),
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn bound_generics() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum A<K: Key, V, U> where V: Value {
                B {
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, Vec<U>),
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn recursive_enum() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum A<K: Key, V> where V: Value {
                B {
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, Vec<A>),
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_borsh_skip_struct_field() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum A<K: Key, V, U> where V: Value {
                B {
                    #[borsh_skip]
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, Vec<U>),
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_borsh_skip_tuple_field() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum A<K: Key, V, U> where V: Value {
                B {
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, #[borsh_skip] Vec<U>),
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_serialize_bound() {
        let item_struct: ItemEnum = syn::parse2(quote! {
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
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn check_serialize_with_attr() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum C<K: Ord, V> {
                C3(u64, u64),
                C4 {
                    x: u64,
                    #[borsh(serialize_with = "third_party_impl::serialize_third_party")]
                    y: ThirdParty<K, V>
                },
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn borsh_discriminant_false() {
        let item_enum: ItemEnum = syn::parse2(quote! {
           #[borsh(use_discriminant = false)]
            enum X {
                A,
                B = 20,
                C,
                D,
                E = 10,
                F,
            }
        })
        .unwrap();
        let actual = process(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }
    #[test]
    fn borsh_discriminant_true() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            #[borsh(use_discriminant = true)]
            enum X {
                A,
                B = 20,
                C,
                D,
                E = 10,
                F,
            }
        })
        .unwrap();
        let actual = process(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }
}
