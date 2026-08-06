use std::collections::HashMap;

use proc_macro2::{Literal, Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{
    Attribute, BinOp, Error, Expr, ExprLit, ExprPath, Ident, ItemEnum, Lit, Meta, Path, Result,
    Visibility,
};

use crate::model::{FlagDirective, FlagVariant};

/// Derives implemented by the macro itself; user-provided duplicates are dropped.
const RESERVED_DERIVES: &[&str] = &["Copy", "Clone", "PartialEq", "Eq", "Debug"];

/// Explicit backing integer type requested via `#[flags(u32)]`.
pub(crate) struct BackingType {
    ident: Ident,
    max: u128,
}

impl Parse for BackingType {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident: Ident = input.parse()?;
        let max = match ident.to_string().as_str() {
            "u8" => u8::MAX as u128,
            "u16" => u16::MAX as u128,
            "u32" => u32::MAX as u128,
            "u64" => u64::MAX as u128,
            "u128" => u128::MAX,
            other => {
                return Err(Error::new_spanned(
                    &ident,
                    format!("unsupported backing type `{other}`, expected one of u8, u16, u32, u64, u128"),
                ));
            }
        };
        Ok(Self { ident, max })
    }
}

/// A fully parsed `#[flags]` input enum.
pub(crate) struct FlagsInput {
    pub(crate) vis: Visibility,
    pub(crate) ident: syn::Ident,
    pub(crate) variants: Vec<FlagVariant>,
    /// User-provided derives forwarded to the generated type (e.g. `Serialize`).
    pub(crate) forwarded_derives: Vec<Path>,
    /// Other container attributes forwarded as-is (e.g. `#[serde(...)]`, docs).
    pub(crate) forwarded_attrs: Vec<Attribute>,
    /// Explicit backing type ident requested via `#[flags(u32)]`, if any.
    pub(crate) explicit_backing: Option<Ident>,
}

struct RawFlagVariant {
    ident: syn::Ident,
    directive: Option<FlagDirective>,
    discriminant_expr: Option<Expr>,
}

/// The result of resolving a variant's value expression.
#[derive(Clone)]
struct ResolvedVariant {
    /// The value folded down to an integer literal, when every referenced
    /// name is a sibling flag with a known literal value.
    literal: Option<u128>,
    /// Rust expression tokens computing the value, with sibling flag-name
    /// references rewritten to `Self::Name.0`.
    tokens: TokenStream,
}

#[derive(Clone, Copy)]
enum CacheState {
    Unresolved,
    Resolving,
}

struct Resolver {
    raw_variants: Vec<RawFlagVariant>,
    indexes: HashMap<String, usize>,
    states: Vec<CacheState>,
    resolved: Vec<Option<ResolvedVariant>>,
}

/// Parses the source enum and resolves every flag value.
pub(crate) fn parse_flags(input: ItemEnum, backing: Option<BackingType>) -> Result<FlagsInput> {
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
            directive: parse_flag_attr(flag_attr)?,
            discriminant_expr: variant.discriminant.map(|(_, expr)| expr),
        });
    }

    let ignored_count = raw_variants
        .iter()
        .filter(|variant| matches!(variant.directive, Some(FlagDirective::Ignore)))
        .count();
    if ignored_count == raw_variants.len() {
        return Err(Error::new(
            Span::call_site(),
            "flags enum must have at least one non-ignored variant",
        ));
    }

    let mut resolver = Resolver::new(raw_variants)?;
    let mut variants = Vec::new();

    for index in 0..resolver.raw_variants.len() {
        if matches!(
            resolver.raw_variants[index].directive,
            Some(FlagDirective::Ignore)
        ) {
            continue;
        }
        let directive = resolver.raw_variants[index].directive.clone();
        let ident = resolver.raw_variants[index].ident.clone();
        let resolved = resolver.resolve_variant(index)?;
        variants.push(FlagVariant {
            ident,
            directive: directive.unwrap_or(FlagDirective::Auto),
            value_literal: resolved.literal,
            value_tokens: resolved.tokens,
        });
    }

    let explicit_backing = if let Some(backing) = backing {
        let max_value = variants
            .iter()
            .filter_map(|v| v.value_literal)
            .max()
            .unwrap_or(0);
        if max_value > backing.max {
            return Err(Error::new_spanned(
                &backing.ident,
                format!(
                    "flag value {max_value:#x} does not fit in `{}`",
                    backing.ident
                ),
            ));
        }
        Some(backing.ident)
    } else {
        for variant in &variants {
            if variant.value_literal.is_none() {
                return Err(Error::new_spanned(
                    &variant.ident,
                    "flag value is not a constant integer; specify an explicit backing type, e.g. #[flags(u32)]",
                ));
            }
        }
        None
    };

    Ok(FlagsInput {
        vis,
        ident,
        variants,
        forwarded_derives,
        forwarded_attrs,
        explicit_backing,
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

        let states = vec![CacheState::Unresolved; raw_variants.len()];
        let resolved = vec![None; raw_variants.len()];

        Ok(Self {
            raw_variants,
            indexes,
            states,
            resolved,
        })
    }

    fn resolve_variant(&mut self, index: usize) -> Result<ResolvedVariant> {
        if let Some(FlagDirective::Ignore) = self.raw_variants[index].directive {
            return Ok(ResolvedVariant {
                literal: Some(0),
                tokens: quote! { 0 },
            });
        }
        if let Some(resolved) = &self.resolved[index] {
            return Ok(resolved.clone());
        }
        if let CacheState::Resolving = self.states[index] {
            return Err(Error::new_spanned(
                &self.raw_variants[index].ident,
                "cyclic flag value reference",
            ));
        }

        self.states[index] = CacheState::Resolving;

        let directive = self.raw_variants[index].directive.clone();
        let discriminant_expr = self.raw_variants[index].discriminant_expr.clone();
        let span = self.raw_variants[index].ident.span();

        let explicit = match directive {
            Some(FlagDirective::Auto) | None => None,
            Some(FlagDirective::Value(expr)) => Some(self.resolve_expr(&expr)?),
            Some(FlagDirective::Ignore) => unreachable!(),
        };
        let discriminant = match discriminant_expr.as_ref() {
            Some(expr) => Some(self.resolve_expr(expr)?),
            None => None,
        };

        let resolved = match (explicit, discriminant) {
            (Some(attr), Some(discriminant)) => {
                if let (Some(a), Some(d)) = (attr.literal, discriminant.literal) {
                    if a != d {
                        return Err(Error::new(
                            span,
                            "#[flag(value)] and discriminant value differ",
                        ));
                    }
                }
                attr
            }
            (Some(attr), None) => attr,
            (None, Some(discriminant)) => discriminant,
            (None, None) => {
                let previous = if index == 0 {
                    None
                } else {
                    Some(self.resolve_variant(index - 1)?)
                };
                let value = next_auto_value(previous.and_then(|p| p.literal), span)?;
                let literal = Literal::u128_unsuffixed(value);
                ResolvedVariant {
                    literal: Some(value),
                    tokens: quote! { #literal },
                }
            }
        };

        self.resolved[index] = Some(resolved.clone());
        Ok(resolved)
    }

    fn resolve_expr(&mut self, expr: &Expr) -> Result<ResolvedVariant> {
        match expr {
            Expr::Lit(ExprLit {
                lit: Lit::Int(lit), ..
            }) => {
                let value = lit.base10_parse::<u128>()?;
                let literal = Literal::u128_unsuffixed(value);
                Ok(ResolvedVariant {
                    literal: Some(value),
                    tokens: quote! { #literal },
                })
            }
            Expr::Path(path) => self.resolve_path(path),
            Expr::Binary(binary) if matches!(binary.op, BinOp::BitOr(_)) => {
                let left = self.resolve_expr(&binary.left)?;
                let right = self.resolve_expr(&binary.right)?;
                let literal = match (left.literal, right.literal) {
                    (Some(a), Some(b)) => Some(a | b),
                    _ => None,
                };
                let (left_tokens, right_tokens) = (left.tokens, right.tokens);
                Ok(ResolvedVariant {
                    literal,
                    tokens: quote! { (#left_tokens) | (#right_tokens) },
                })
            }
            Expr::Paren(paren) => self.resolve_expr(&paren.expr),
            Expr::Group(group) => self.resolve_expr(&group.expr),
            _ => Err(Error::new_spanned(
                expr,
                "expected an integer literal, flag name, external constant path, or `|` expression",
            )),
        }
    }

    fn resolve_path(&mut self, path: &ExprPath) -> Result<ResolvedVariant> {
        if path.qself.is_some() {
            return Err(Error::new_spanned(path, "expected a flag variant name"));
        }

        let Some(ident) = path.path.get_ident() else {
            // A multi-segment path (e.g. `module::CONST`) can only refer to
            // an external item; forward it unresolved.
            return Ok(ResolvedVariant {
                literal: None,
                tokens: quote! { #path },
            });
        };

        if let Some(&index) = self.indexes.get(&ident.to_string()) {
            let resolved = self.resolve_variant(index)?;
            return Ok(ResolvedVariant {
                literal: resolved.literal,
                tokens: quote! { Self::#ident.0 },
            });
        }

        // Not a sibling flag name; treat as an external constant whose value
        // only the final Rust compilation can know.
        Ok(ResolvedVariant {
            literal: None,
            tokens: quote! { #ident },
        })
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

fn parse_flag_attr(attr: &Attribute) -> Result<Option<FlagDirective>> {
    match &attr.meta {
        Meta::Path(_) => Ok(Some(FlagDirective::Auto)),
        Meta::List(list) => Ok(Some(parse_flag_expr(list.parse_args::<Expr>()?)?)),
        Meta::NameValue(_) => Err(Error::new_spanned(
            attr,
            "expected #[flag], #[flag(ignore)], or #[flag(value)]",
        )),
    }
}

fn parse_flag_expr(expr: Expr) -> Result<FlagDirective> {
    if let Expr::Path(ExprPath {
        qself: None, path, ..
    }) = &expr
    {
        if path.is_ident("ignore") {
            return Ok(FlagDirective::Ignore);
        }
    }
    Ok(FlagDirective::Value(expr))
}

fn next_auto_value(previous: Option<u128>, span: Span) -> Result<u128> {
    match previous {
        None => Ok(0),
        Some(0) => Ok(1),
        Some(value) if value.is_power_of_two() => Ok(value << 1),
        Some(_) => Err(Error::new(
            span,
            "cannot auto-assign flag value after a value that is composite or not a constant integer",
        )),
    }
}
