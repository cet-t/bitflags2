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
#[proc_macro_attribute]
pub fn flags(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemEnum);

    match parse::parse_flags(input) {
        Ok(flags) => codegen::generate(flags).into(),
        Err(error) => error.to_compile_error().into(),
    }
}
