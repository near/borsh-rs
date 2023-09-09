use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Fields, Ident, ItemEnum, Path, Variant};

use crate::internals::{
    attributes::{field, item, BoundType},
    enum_discriminant::Discriminants,
    generics, serialize,
};

pub fn process(input: &ItemEnum, cratename: Path) -> syn::Result<TokenStream2> {
    let enum_ident = &input.ident;
    let generics = generics::without_defaults(&input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut where_clause = generics::default_where(where_clause);
    let mut generics_output = serialize::GenericsOutput::new(&generics);
    let mut all_variants_idx_body = TokenStream2::new();
    let mut fields_body = TokenStream2::new();
    let use_discriminant = item::contains_use_discriminant(input)?;
    let discriminants = Discriminants::new(&input.variants);

    for (variant_idx, variant) in input.variants.iter().enumerate() {
        let variant_ident = &variant.ident;
        let discriminant_value = discriminants.get(variant_ident, use_discriminant, variant_idx)?;
        let variant_output = process_variant(
            variant,
            enum_ident,
            &discriminant_value,
            &cratename,
            &mut generics_output,
        )?;
        all_variants_idx_body.extend(variant_output.variant_idx_body);
        let (variant_header, variant_body) = (variant_output.header, variant_output.body);
        fields_body.extend(quote!(
            #enum_ident::#variant_ident #variant_header => {
                #variant_body
            }
        ))
    }
    generics_output.extend(&mut where_clause, &cratename);

    Ok(quote! {
        impl #impl_generics #cratename::ser::BorshSerialize for #enum_ident #ty_generics #where_clause {
            fn serialize<W: #cratename::io::Write>(&self, writer: &mut W) -> ::core::result::Result<(), #cratename::io::Error> {
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
    cratename: &Path,
    generics: &mut serialize::GenericsOutput,
) -> syn::Result<VariantOutput> {
    let variant_ident = &variant.ident;
    let mut variant_output = VariantOutput::new();
    match &variant.fields {
        Fields::Named(fields) => {
            for field in &fields.named {
                let field_id = serialize::FieldId::Enum(field.ident.clone().unwrap());
                process_field(field, field_id, cratename, generics, &mut variant_output)?;
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
                let field_id = serialize::FieldId::new_enum_unnamed(field_idx)?;
                process_field(field, field_id, cratename, generics, &mut variant_output)?;
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
    field_id: serialize::FieldId,
    cratename: &Path,
    generics: &mut serialize::GenericsOutput,
    output: &mut VariantOutput,
) -> syn::Result<()> {
    let parsed = field::Attributes::parse(&field.attrs)?;

    let needs_bounds_derive = parsed.needs_bounds_derive(BoundType::Serialize);
    generics
        .overrides
        .extend(parsed.collect_bounds(BoundType::Serialize));

    let field_variant_header = field_id.enum_variant_header(parsed.skip);
    if let Some(field_variant_header) = field_variant_header {
        output.header.extend(field_variant_header);
    }

    if !parsed.skip {
        let delta = field_id.serialize_output(cratename, parsed.serialize_with);
        output.body.extend(delta);
        if needs_bounds_derive {
            generics.serialize_visitor.visit_field(field);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::internals::test_helpers::{
        default_cratename, local_insta_assert_snapshot, pretty_print_syn_str,
    };

    use super::*;
    #[test]
    fn borsh_skip_tuple_variant_field() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            enum AATTB {
                B(#[borsh(skip)] i32, #[borsh(skip)] u32),

                NegatedVariant {
                    beta: u8,
                }
            }
        })
        .unwrap();
        let actual = process(&item_enum, default_cratename()).unwrap();

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

        let actual = process(&item_enum, default_cratename()).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn simple_enum_with_custom_crate() {
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

        let crate_: Path = syn::parse2(quote! { reexporter::borsh }).unwrap();
        let actual = process(&item_enum, crate_).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn borsh_skip_struct_variant_field() {
        let item_enum: ItemEnum = syn::parse2(quote! {

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
        })
        .unwrap();

        let actual = process(&item_enum, default_cratename()).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn borsh_skip_struct_variant_all_fields() {
        let item_enum: ItemEnum = syn::parse2(quote! {

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
        })
        .unwrap();

        let actual = process(&item_enum, default_cratename()).unwrap();

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

        let actual = process(&item_struct, default_cratename()).unwrap();
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

        let actual = process(&item_struct, default_cratename()).unwrap();
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

        let actual = process(&item_struct, default_cratename()).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_borsh_skip_struct_field() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum A<K: Key, V, U> where V: Value {
                B {
                    #[borsh(skip)]
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, Vec<U>),
            }
        })
        .unwrap();

        let actual = process(&item_struct, default_cratename()).unwrap();

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
                C(K, #[borsh(skip)] Vec<U>),
            }
        })
        .unwrap();

        let actual = process(&item_struct, default_cratename()).unwrap();

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

        let actual = process(&item_struct, default_cratename()).unwrap();

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

        let actual = process(&item_struct, default_cratename()).unwrap();

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
        let actual = process(&item_enum, default_cratename()).unwrap();

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
        let actual = process(&item_enum, default_cratename()).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }
}
