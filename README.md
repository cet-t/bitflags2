# bitflags2

`bitflags2` is an attribute-macro based bitflags crate for Rust. It lets you declare flags with enum syntax while generating an integer-backed flag type that supports composite values.

## Features

- Declare flags with familiar enum syntax.
- Use `#[flag]`, `#[flag(value)]`, discriminants, or composite flag expressions.
- Combine flags with `|`, `&`, `^`, `!` and assignment variants.
- Compare and convert with `u8`, `u16`, `u32`, `u64`, and `u128`.
- Print enabled flag names with `Debug`.
- Optional `serde` integration via the `serde` feature.

## Usage

```toml
[dependencies]
bitflags2 = { version = "0.1", features = ["serde"] }
```

```rust
use bitflags2::flags;

#[flags]
enum Permission {
    #[flag]
    None = 0x0000,
    #[flag(Read | Write | Execute)]
    All,
    #[flag(0x0002)]
    Read,
    #[flag(0x0004)]
    Write,
    #[flag(0x0008)]
    Execute,
}

let mut perm = Permission::None;
perm |= Permission::Read | Permission::Write;

assert!(perm.has_flag(Permission::Read));
assert!(Permission::All.has_flag(Permission::Execute));
assert_eq!(perm, 0x0006u32);
assert_eq!(u128::from(perm), 0x0006);
assert_eq!(format!("{:?}", perm), "Read | Write");
```

## Flag values

Each variant must be marked with `#[flag]` or `#[flag(...)]`.

```rust
# use bitflags2::flags;
#[flags]
enum Example {
    #[flag]
    None = 0,
    #[flag]
    Read,
    #[flag]
    Write,
    #[flag(Read | Write)]
    ReadWrite,
}

assert_eq!(Example::None, 0u8);
assert_eq!(Example::Read, 1u8);
assert_eq!(Example::Write, 2u8);
assert_eq!(Example::ReadWrite, 3u8);
```

Value resolution order:

1. `#[flag(value)]`
2. `Variant = value`
3. automatic assignment

Automatic assignment starts with `0` for the first unvalued flag, then shifts by one bit from the previous single-bit value.

## serde support

Enable the `serde` feature to derive `Serialize` and `Deserialize` for generated flag types:

```toml
[dependencies]
bitflags2 = { version = "0.1", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
```

```rust
use bitflags2::flags;
use serde::{Deserialize, Serialize};

#[flags]
enum Permission {
    #[flag]
    None = 0x0000,
    #[flag(0x0002)]
    Read,
    #[flag(0x0004)]
    Write,
}

#[derive(Serialize, Deserialize)]
struct Permissions {
    perm: Permission,
}

let p = Permissions {
    perm: Permission::Read | Permission::Write,
};
let json = serde_json::to_string(&p).unwrap();
let back: Permissions = serde_json::from_str(&json).unwrap();
assert_eq!(back.perm, p.perm);
```

## Generated API

For each `#[flags] enum Name { ... }`, `bitflags2` generates an integer-backed `Name` type with:

- associated constants for each variant
- `empty()`
- `bits()`
- `from_bits(bits)`
- `has_flag(other)`
- `BitOr`, `BitAnd`, `BitXor`, `Not` and assignment variants
- `From<u8/u16/u32/u64/u128>` and reverse conversions
- `PartialEq<u8/u16/u32/u64/u128>` in both directions
- `Copy`, `Clone`, `PartialEq`, `Eq`, and `Debug`

When the `serde` feature is enabled, the generated type also implements `Serialize` and `Deserialize`.

## Notes

`#[flag]` is a helper marker consumed by `#[flags]`. It is not exported as a standalone attribute, so using `#[flag]` outside a `#[flags]` enum fails at compile time.

The generated type currently stores bits as `u128`. Converting to smaller integer widths uses Rust's `as` conversion semantics.
