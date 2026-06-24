use proc_macro2::Ident;

/// Parsed flag definition used by the code generator.
pub(crate) struct FlagVariant {
    pub(crate) ident: Ident,
    pub(crate) value: u128,
}
