use proc_macro2::{Ident, TokenStream};
use syn::Expr;

/// Directives parsed from a `#[flag(...)]` attribute.
#[derive(Clone)]
pub(crate) enum FlagDirective {
    Auto,
    Value(Expr),
    Ignore,
}

/// Parsed flag definition used by the code generator.
pub(crate) struct FlagVariant {
    pub(crate) ident: Ident,
    pub(crate) directive: FlagDirective,
    /// The variant's value folded to an integer literal, when every
    /// referenced name resolves to a known constant. `None` when the value
    /// depends on an external item (e.g. `Admin = SOME_EXTERNAL_CONST`)
    /// whose value only the final Rust compilation can know.
    pub(crate) value_literal: Option<u128>,
    /// Rust expression tokens computing the variant's value, with references
    /// to sibling flag names rewritten to `Self::Name.0`.
    pub(crate) value_tokens: TokenStream,
}

impl FlagVariant {
    pub(crate) fn is_ignored(&self) -> bool {
        matches!(self.directive, FlagDirective::Ignore)
    }
}
