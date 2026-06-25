use std::collections::HashMap;

use proc_macro2::Span;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{
    Attribute, BinOp, Error, Expr, ExprLit, ExprPath, ItemEnum, Lit, Meta, Path, Result,
    Visibility,
};

use crate::model::FlagVariant;

/// Derives implemented by the macro itself; user-provided duplicates are dropped.
const RESERVED_DERIVES: &[&str] = &["Copy", "Clone", "PartialEq", "Eq", "Debug"];

/// A fully parsed `#[flags]` input enum.
pub(crate) struct FlagsInput {
    pub(crate) vis: Visibility,
    pub(crate) ident: syn::Ident,
    pub(crate) variants: Vec<FlagVariant>,
    /// User-provided derives forwarded to the generated type (e.g. `Serialize`).
    pub(crate) forwarded_derives: Vec<Path>,
    /// Other container attributes forwarded as-is (e.g. `#[serde(...)]`, docs).
    pub(crate) forwarded_attrs: Vec<Attribute>,
}

struct RawFlagVariant {
    ident: syn::Ident,
    attr_expr: Option<Expr>,
    discriminant_expr: Option<Expr>,
}

#[derive(Clone, Copy)]
enum ResolveState {
    Unresolved,
    Resolving,
    Resolved(u128),
}

struct Resolver {
    raw_variants: Vec<RawFlagVariant>,
    indexes: HashMap<String, usize>,
    states: Vec<ResolveState>,
}

/// Parses the source enum and resolves every flag value.
pub(crate) fn parse_flags(input: ItemEnum) -> Result<FlagsInput> {
    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            input.generics,
            "flags enum cannot be generic",
        ));
    }

    let vis = input.vis;
    let ident = input.ident;
    let (forwarded_derives, forwarded_attrs) = split_container_attrs(input.attrs)?;
    let mut raw_variants = Vec::new();

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

        raw_variants.push(RawFlagVariant {
            ident: variant.ident,
            attr_expr: parse_flag_attr(flag_attr)?,
            discriminant_expr: variant.discriminant.map(|(_, expr)| expr),
        });
    }

    let mut resolver = Resolver::new(raw_variants)?;
    let mut variants = Vec::new();

    for index in 0..resolver.raw_variants.len() {
        let value = resolver.resolve_variant(index)?;
        variants.push(FlagVariant {
            ident: resolver.raw_variants[index].ident.clone(),
            value,
        });
    }

    Ok(FlagsInput {
        vis,
        ident,
        variants,
        forwarded_derives,
        forwarded_attrs,
    })
}

/// Splits the enum's outer attributes into forwarded derives and other attributes.
///
/// Derives implemented directly by this macro (`Copy`, `Clone`, `PartialEq`,
/// `Eq`, `Debug`) are dropped to avoid conflicting impls; everything else is
/// forwarded to the generated newtype so that, for example, `#[derive(Serialize,
/// Deserialize)]` keeps working on the flag type directly.
fn split_container_attrs(attrs: Vec<Attribute>) -> Result<(Vec<Path>, Vec<Attribute>)> {
    let mut derives = Vec::new();
    let mut others = Vec::new();

    for attr in attrs {
        if attr.path().is_ident("derive") {
            let paths = attr.parse_args_with(Punctuated::<Path, Comma>::parse_terminated)?;
            for path in paths {
                if !is_reserved_derive(&path) {
                    derives.push(path);
                }
            }
        } else {
            others.push(attr);
        }
    }

    Ok((derives, others))
}

/// Returns true for derives the macro implements directly and must not forward.
fn is_reserved_derive(path: &Path) -> bool {
    let Some(last) = path.segments.last() else {
        return false;
    };
    RESERVED_DERIVES.contains(&last.ident.to_string().as_str())
}

impl Resolver {
    fn new(raw_variants: Vec<RawFlagVariant>) -> Result<Self> {
        let mut indexes = HashMap::new();

        for (index, variant) in raw_variants.iter().enumerate() {
            if indexes.insert(variant.ident.to_string(), index).is_some() {
                return Err(Error::new_spanned(
                    &variant.ident,
                    "duplicate flag variant name",
                ));
            }
        }

        let states = vec![ResolveState::Unresolved; raw_variants.len()];

        Ok(Self {
            raw_variants,
            indexes,
            states,
        })
    }

    fn resolve_variant(&mut self, index: usize) -> Result<u128> {
        match self.states[index] {
            ResolveState::Resolved(value) => return Ok(value),
            ResolveState::Resolving => {
                return Err(Error::new_spanned(
                    &self.raw_variants[index].ident,
                    "cyclic flag value reference",
                ));
            }
            ResolveState::Unresolved => {}
        }

        self.states[index] = ResolveState::Resolving;

        let attr_expr = self.raw_variants[index].attr_expr.clone();
        let discriminant_expr = self.raw_variants[index].discriminant_expr.clone();
        let span = self.raw_variants[index].ident.span();

        let attr_value = match attr_expr.as_ref() {
            Some(expr) => Some(self.eval_flag_expr(expr)?),
            None => None,
        };
        let discriminant_value = match discriminant_expr.as_ref() {
            Some(expr) => Some(parse_int_expr(expr)?),
            None => None,
        };

        let value = match (attr_value, discriminant_value) {
            (Some(attr), Some(discriminant)) if attr != discriminant => {
                return Err(Error::new(
                    span,
                    "#[flag(value)] and discriminant value differ",
                ));
            }
            (Some(attr), _) => attr,
            (None, Some(discriminant)) => discriminant,
            (None, None) => {
                let previous = if index == 0 {
                    None
                } else {
                    Some(self.resolve_variant(index - 1)?)
                };
                next_auto_value(previous, span)?
            }
        };

        self.states[index] = ResolveState::Resolved(value);
        Ok(value)
    }

    fn eval_flag_expr(&mut self, expr: &Expr) -> Result<u128> {
        match expr {
            Expr::Lit(ExprLit {
                lit: Lit::Int(lit), ..
            }) => lit.base10_parse::<u128>(),
            Expr::Path(path) => self.eval_path(path),
            Expr::Binary(binary) if matches!(binary.op, BinOp::BitOr(_)) => {
                Ok(self.eval_flag_expr(&binary.left)? | self.eval_flag_expr(&binary.right)?)
            }
            Expr::Paren(paren) => self.eval_flag_expr(&paren.expr),
            Expr::Group(group) => self.eval_flag_expr(&group.expr),
            _ => Err(Error::new_spanned(
                expr,
                "expected an integer literal, flag name, or `|` expression",
            )),
        }
    }

    fn eval_path(&mut self, path: &ExprPath) -> Result<u128> {
        if path.qself.is_some() {
            return Err(Error::new_spanned(path, "expected a flag variant name"));
        }

        let Some(ident) = path.path.get_ident() else {
            return Err(Error::new_spanned(path, "expected a flag variant name"));
        };

        let Some(index) = self.indexes.get(&ident.to_string()).copied() else {
            return Err(Error::new_spanned(ident, "unknown flag variant name"));
        };

        self.resolve_variant(index)
    }
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

fn parse_flag_attr(attr: &Attribute) -> Result<Option<Expr>> {
    match &attr.meta {
        Meta::Path(_) => Ok(None),
        Meta::List(list) => Ok(Some(list.parse_args::<Expr>()?)),
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
