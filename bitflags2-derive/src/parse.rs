use proc_macro2::Span;
use syn::{Attribute, Error, Expr, ExprLit, ItemEnum, Lit, Meta, Result, Visibility};

use crate::model::FlagVariant;

/// A fully parsed `#[flags]` input enum.
pub(crate) struct FlagsInput {
    pub(crate) vis: Visibility,
    pub(crate) ident: syn::Ident,
    pub(crate) variants: Vec<FlagVariant>,
}

/// Parses the source enum and resolves every flag value.
pub(crate) fn parse_flags(input: ItemEnum) -> Result<FlagsInput> {
    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            input.generics,
            "flags enum cannot be generic",
        ));
    }

    let mut variants = Vec::new();
    let mut previous = None;

    for variant in input.variants {
        if !variant.fields.is_empty() {
            return Err(Error::new_spanned(
                variant.fields,
                "flags variants cannot have fields",
            ));
        }

        let flag_attr = flag_attr(&variant.attrs)?;
        let Some(flag_attr) = flag_attr else {
            return Err(Error::new_spanned(
                variant.ident,
                "flags enum variants must have #[flag] or #[flag(value)]",
            ));
        };

        let attr_value = parse_flag_attr(flag_attr)?;
        let discriminant_value = match &variant.discriminant {
            Some((_, expr)) => Some(parse_int_expr(expr)?),
            None => None,
        };

        let value = match (attr_value, discriminant_value) {
            (Some(attr), Some(discriminant)) if attr != discriminant => {
                return Err(Error::new_spanned(
                    variant,
                    "#[flag(value)] and discriminant value differ",
                ));
            }
            (Some(attr), _) => attr,
            (None, Some(discriminant)) => discriminant,
            (None, None) => next_auto_value(previous, Span::call_site())?,
        };

        previous = Some(value);
        variants.push(FlagVariant {
            ident: variant.ident,
            value,
        });
    }

    Ok(FlagsInput {
        vis: input.vis,
        ident: input.ident,
        variants,
    })
}

fn flag_attr(attrs: &[Attribute]) -> Result<Option<&Attribute>> {
    let mut found = None;

    for attr in attrs {
        if attr.path().is_ident("flag") {
            if found.is_some() {
                return Err(Error::new_spanned(attr, "duplicate #[flag] attribute"));
            }
            found = Some(attr);
        }
    }

    Ok(found)
}

fn parse_flag_attr(attr: &Attribute) -> Result<Option<u128>> {
    match &attr.meta {
        Meta::Path(_) => Ok(None),
        Meta::List(list) => {
            let expr = list.parse_args::<Expr>()?;
            Ok(Some(parse_int_expr(&expr)?))
        }
        Meta::NameValue(_) => Err(Error::new_spanned(
            attr,
            "expected #[flag] or #[flag(value)]",
        )),
    }
}

fn parse_int_expr(expr: &Expr) -> Result<u128> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Int(lit), ..
        }) => lit.base10_parse::<u128>(),
        _ => Err(Error::new_spanned(expr, "expected integer literal")),
    }
}

fn next_auto_value(previous: Option<u128>, span: Span) -> Result<u128> {
    match previous {
        None => Ok(0),
        Some(0) => Ok(1),
        Some(value) if value.is_power_of_two() => Ok(value << 1),
        Some(_) => Err(Error::new(
            span,
            "cannot auto-assign flag value after a composite value",
        )),
    }
}
