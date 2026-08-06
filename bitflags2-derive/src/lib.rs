use proc_macro::TokenStream;
use syn::{ItemEnum, parse_macro_input};

mod codegen;
mod model;
mod parse;

/// Expands an enum into a compact bitflag type.
///
/// The `#[flag]` attributes on variants are helper markers consumed by this
/// attribute macro. They are intentionally not exported as standalone macros,
/// so using `#[flag]` without `#[flags]` fails at compile time.
///
/// `#[flags]` optionally accepts an explicit backing integer type, e.g.
/// `#[flags(u32)]`. Without an argument the smallest type that fits every
/// flag value is chosen automatically, matching prior behavior.
#[proc_macro_attribute]
pub fn flags(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemEnum);

    let backing = if attr.is_empty() {
        None
    } else {
        Some(parse_macro_input!(attr as parse::BackingType))
    };

    match parse::parse_flags(input, backing) {
        Ok(flags) => codegen::generate(flags).into(),
        Err(error) => error.to_compile_error().into(),
    }
}
