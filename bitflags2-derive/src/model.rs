use proc_macro2::Ident;
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
    pub(crate) value: Option<u128>,
}

impl FlagVariant {
    pub(crate) fn is_ignored(&self) -> bool {
        matches!(self.directive, FlagDirective::Ignore)
    }
}
