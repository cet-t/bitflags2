use proc_macro2::{Literal, TokenStream as TokenStream2};
use quote::{format_ident, quote};

use crate::parse::FlagsInput;

/// Generates the public flag type and all supported operations.
pub(crate) fn generate(input: FlagsInput) -> TokenStream2 {
    let vis = input.vis;
    let enum_ident = input.ident;
    let variants = input.variants;
    let forwarded_attrs = input.forwarded_attrs;
    let forwarded_derives = input.forwarded_derives;

    let derive_attr = derive_attr(forwarded_derives);

    let consts = variants.iter().map(|variant| {
        let ident = &variant.ident;
        let value = u128_literal(variant.value);
        quote! {
            #[allow(non_upper_case_globals)]
            pub const #ident: Self = Self(#value);
        }
    });

    let debug_arms = variants
        .iter()
        .filter(|variant| variant.value != 0)
        .map(|variant| {
            let name = variant.ident.to_string();
            let value = u128_literal(variant.value);
            quote! {
                if self.has_flag(Self(#value)) {
                    if !first {
                        f.write_str(" | ")?;
                    }
                    f.write_str(#name)?;
                    first = false;
                }
            }
        });

    let zero_name = variants
        .iter()
        .find(|variant| variant.value == 0)
        .map(|variant| variant.ident.to_string())
        .unwrap_or_else(|| "0".to_string());

    let impl_ints_macro = format_ident!("__bitflags2_impl_ints_for_{}", enum_ident);

    quote! {
        #(#forwarded_attrs)*
        #derive_attr
        #vis struct #enum_ident(u128);

        impl #enum_ident {
            #(#consts)*

            pub const fn empty() -> Self {
                Self(0)
            }

            pub const fn bits(self) -> u128 {
                self.0
            }

            pub const fn from_bits(bits: u128) -> Self {
                Self(bits)
            }

            pub const fn has_flag(self, other: Self) -> bool {
                (self.0 & other.0) == other.0
            }
        }

        macro_rules! #impl_ints_macro {
            ($($ty:ty),* $(,)?) => {
                $(
                    impl ::core::convert::From<$ty> for #enum_ident {
                        fn from(bits: $ty) -> Self {
                            Self(bits as u128)
                        }
                    }

                    impl ::core::convert::From<#enum_ident> for $ty {
                        fn from(flags: #enum_ident) -> Self {
                            flags.0 as $ty
                        }
                    }

                    impl ::core::cmp::PartialEq<$ty> for #enum_ident {
                        fn eq(&self, rhs: &$ty) -> bool {
                            self.0 == *rhs as u128
                        }
                    }

                    impl ::core::cmp::PartialEq<#enum_ident> for $ty {
                        fn eq(&self, rhs: &#enum_ident) -> bool {
                            *self as u128 == rhs.0
                        }
                    }
                )*
            };
        }

        #impl_ints_macro!(u8, u16, u32, u64, u128);

        impl ::core::ops::BitOr for #enum_ident {
            type Output = Self;

            fn bitor(self, rhs: Self) -> Self::Output {
                Self(self.0 | rhs.0)
            }
        }

        impl ::core::ops::BitOrAssign for #enum_ident {
            fn bitor_assign(&mut self, rhs: Self) {
                self.0 |= rhs.0;
            }
        }

        impl ::core::ops::BitAnd for #enum_ident {
            type Output = Self;

            fn bitand(self, rhs: Self) -> Self::Output {
                Self(self.0 & rhs.0)
            }
        }

        impl ::core::ops::BitAndAssign for #enum_ident {
            fn bitand_assign(&mut self, rhs: Self) {
                self.0 &= rhs.0;
            }
        }

        impl ::core::ops::BitXor for #enum_ident {
            type Output = Self;

            fn bitxor(self, rhs: Self) -> Self::Output {
                Self(self.0 ^ rhs.0)
            }
        }

        impl ::core::ops::BitXorAssign for #enum_ident {
            fn bitxor_assign(&mut self, rhs: Self) {
                self.0 ^= rhs.0;
            }
        }

        impl ::core::ops::Not for #enum_ident {
            type Output = Self;

            fn not(self) -> Self::Output {
                Self(!self.0)
            }
        }

        impl ::core::fmt::Debug for #enum_ident {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                let mut first = true;
                #(#debug_arms)*
                if first {
                    f.write_str(#zero_name)?;
                }
                Ok(())
            }
        }
    }
}

fn derive_attr(forwarded_derives: Vec<syn::Path>) -> TokenStream2 {
    if forwarded_derives.is_empty() {
        quote! { #[derive(Copy, Clone, PartialEq, Eq)] }
    } else {
        quote! { #[derive(Copy, Clone, PartialEq, Eq, #(#forwarded_derives),*)] }
    }
}

fn u128_literal(value: u128) -> Literal {
    Literal::u128_unsuffixed(value)
}
