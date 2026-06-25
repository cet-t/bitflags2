# bitflags2-derive

`bitflags2-derive` provides the `#[flags]` procedural macro used by the [`bitflags2`](https://crates.io/crates/bitflags2) crate.

Most users should depend on `bitflags2` instead of this crate directly:

```toml
[dependencies]
bitflags2 = "0.1"
```

## What this crate does

The `#[flags]` macro accepts an enum declaration and expands it into an integer-backed flag type. Variant-level `#[flag]` markers are parsed by the macro and are not standalone attributes.

```rust
use bitflags2::flags;

#[flags]
enum Permission {
    #[flag]
    None = 0,
    #[flag(Read | Write)]
    All,
    #[flag]
    Read,
    #[flag]
    Write,
}

assert_eq!(Permission::All, Permission::Read | Permission::Write);
```

## Supported flag expressions

`#[flag(...)]` supports:

- integer literals, such as `#[flag(0x04)]`
- variant names, such as `#[flag(Read)]`
- bitwise OR expressions, such as `#[flag(Read | Write | Execute)]`
- parenthesized/grouped expressions

Forward references to later variants are supported.

## Development

This crate is split into:

- `lib.rs` — procedural macro entry point
- `parse.rs` — enum parsing and value resolution
- `model.rs` — parsed model types
- `codegen.rs` — generated Rust code
