//! Derive-like bitflags powered by an attribute macro.
//!
//! `bitflags2` lets you declare flags with enum syntax while generating a
//! compact integer-backed flag type. Each variant marked with `#[flag]` becomes
//! an associated constant on the generated type.
//!
//! # Example
//!
//! An explicit backing type can be requested with `#[flags(u32)]`; without an
//! argument, the smallest integer type that fits every flag value is chosen.
//!
//! ```
//! use bitflags2::flags;
//!
//! #[flags]
//! enum Permission {
//!     #[flag]
//!     None = 0x0000,
//!     #[flag(Read | Write | Execute)]
//!     All,
//!     #[flag(0x0002)]
//!     Read,
//!     #[flag(0x0004)]
//!     Write,
//!     #[flag(0x0008)]
//!     Execute,
//! }
//!
//! let mut perm = Permission::None;
//! perm |= Permission::Read | Permission::Write;
//!
//! assert!(perm.has_flag(Permission::Read));
//! assert_eq!(perm, 0x0006u32);
//! assert_eq!(format!("{:?}", perm), "Read | Write");
//! ```

/// Generates an integer-backed flag type from an enum declaration.
pub use bitflags2_derive::flags;

#[cfg(test)]
mod tests {
    use super::flags;

    const X: u8 = 0x02;
    #[flags(u8)]
    enum Role {
        #[flag]
        Owner = Manager | Admin | Member,
        #[flag]
        Manager = 0x01,
        #[flag]
        Admin = X,
        #[flag]
        Member = 0x04,
    }

    #[flags]
    enum Permission {
        #[flag(0x0000)]
        None,
        #[flag(0x0002)]
        Read,
        #[flag(0x0004)]
        Write,
        #[flag(0x0008)]
        Execute,
        #[flag(Read | Write | Execute)]
        All,
    }

    #[flags]
    enum AutoAssigned {
        #[flag]
        None,
        #[flag]
        One,
        #[flag]
        Two,
        #[flag]
        Four,
    }

    #[test]
    fn combines_flags_with_bit_or_assign() {
        let mut perm = Permission::None;
        perm |= Permission::Read | Permission::Write;

        assert_eq!(perm.bits(), 0x0006);
        assert_eq!(Permission::All.bits(), 0x000e);
        assert!(Permission::All.has_flag(Permission::Execute));
        assert!(perm.has_flag(Permission::Read));
        assert!(perm.has_flag(Permission::Write));
        assert!(!perm.has_flag(Permission::Execute));
    }

    #[test]
    fn supports_bit_operations() {
        let mut perm = Permission::Read | Permission::Write | Permission::Execute;

        perm &= Permission::Read | Permission::Execute;
        assert_eq!(perm.bits(), 0x000a);

        perm ^= Permission::Execute;
        assert_eq!(perm.bits(), 0x0002);

        assert_eq!((Permission::Read & Permission::Write).bits(), 0x0000);
    }

    #[test]
    fn auto_assigns_shifted_values() {
        assert_eq!(AutoAssigned::None.bits(), 0x0000);
        assert_eq!(AutoAssigned::One.bits(), 0x0001);
        assert_eq!(AutoAssigned::Two.bits(), 0x0002);
        assert_eq!(AutoAssigned::Four.bits(), 0x0004);
    }

    #[test]
    fn formats_debug_names() {
        assert_eq!(format!("{:?}", Permission::None), "None");
        assert_eq!(
            format!("{:?}", Permission::Read | Permission::Write),
            "Read | Write"
        );
    }

    #[test]
    fn constructs_from_bits() {
        let perm = Permission::from_bits(0x0006);

        assert_eq!(perm, Permission::Read | Permission::Write);
    }

    #[test]
    fn converts_to_and_from_u32() {
        assert_eq!(Permission::None, 0x0000u32);
        assert_eq!(0x0002u32, Permission::Read);

        let perm: Permission = 0x0006u32.into();
        let bits: u32 = perm.into();

        assert_eq!(perm, Permission::Read | Permission::Write);
        assert_eq!(bits, 0x0006);
    }

    #[test]
    fn converts_across_integer_widths() {
        assert_eq!(Permission::Read, 0x0002u8);
        assert_eq!(Permission::Read, 0x0002u16);
        assert_eq!(Permission::Read, 0x0002u64);
        assert_eq!(Permission::Read, 0x0002u128);

        let from_u8: Permission = 0x0004u8.into();
        let from_u128: Permission = 0x0004u128.into();
        assert_eq!(from_u8, Permission::Write);
        assert_eq!(from_u128, Permission::Write);

        let as_u8: u8 = Permission::Write.into();
        let as_u16: u16 = Permission::Write.into();
        let as_u64: u64 = Permission::Write.into();
        let as_u128: u128 = Permission::Write.into();
        assert_eq!(as_u8, 0x04);
        assert_eq!(as_u16, 0x04);
        assert_eq!(as_u64, 0x04);
        assert_eq!(as_u128, 0x04);
    }

    #[test]
    fn chooses_smallest_backing_integer_width() {
        #[flags]
        enum U8Backed {
            #[flag(0)]
            None,
            #[flag(0xff)]
            Max,
        }

        #[flags]
        enum U16Backed {
            #[flag]
            None,
            #[flag]
            One,
            #[flag]
            Two,
            #[flag]
            Four,
            #[flag]
            Eight,
            #[flag]
            Sixteen,
            #[flag]
            ThirtyTwo,
            #[flag]
            SixtyFour,
            #[flag]
            OneTwentyEight,
            #[flag]
            TwoFiftySix,
        }

        #[flags]
        enum U32Backed {
            #[flag(0x0001_0000)]
            Bit16,
        }

        #[flags]
        enum U64Backed {
            #[flag(0x0001_0000_0000)]
            Bit32,
        }

        #[flags]
        enum U128Backed {
            #[flag(0x0001_0000_0000_0000_0000)]
            Bit64,
        }

        assert_eq!(::core::mem::size_of::<U8Backed>(), 1);
        assert_eq!(::core::mem::size_of::<U16Backed>(), 2);
        assert_eq!(::core::mem::size_of::<U32Backed>(), 4);
        assert_eq!(::core::mem::size_of::<U64Backed>(), 8);
        assert_eq!(::core::mem::size_of::<U128Backed>(), 16);

        let u8_bits: u8 = U8Backed::Max.bits();
        let u16_bits: u16 = U16Backed::TwoFiftySix.bits();
        let u32_bits: u32 = U32Backed::Bit16.bits();
        let u64_bits: u64 = U64Backed::Bit32.bits();
        let u128_bits: u128 = U128Backed::Bit64.bits();

        assert_eq!(u8_bits, 0xff);
        assert_eq!(u16_bits, 0x0100);
        assert_eq!(u32_bits, 0x0001_0000);
        assert_eq!(u64_bits, 0x0001_0000_0000);
        assert_eq!(u128_bits, 0x0001_0000_0000_0000_0000);
    }

    #[test]
    fn ignores_flag_variant() {
        #[flags]
        enum ReadWrite {
            #[flag]
            None,
            #[flag(ignore)]
            Deprecated,
            #[flag(0x01)]
            Read,
            #[flag(0x02)]
            Write,
        }

        assert_eq!(::core::mem::size_of::<ReadWrite>(), 1);
        assert_eq!(ReadWrite::bits(ReadWrite::None), 0);
        assert_eq!(ReadWrite::bits(ReadWrite::Read), 0x01);
        assert_eq!(ReadWrite::bits(ReadWrite::Write), 0x02);

        assert_eq!(format!("{:?}", ReadWrite::None), "None");
        assert_eq!(
            format!("{:?}", ReadWrite::Read | ReadWrite::Write),
            "Read | Write"
        );

        let from_bits: ReadWrite = ReadWrite::from_bits(0x01);
        assert_eq!(from_bits, ReadWrite::Read);
    }

    #[test]
    fn discriminant_can_reference_flag_names_and_external_consts() {
        assert_eq!(Role::Owner.bits(), 0x07);
        assert_eq!(Role::Admin.bits(), 0x02);
        assert!(Role::Owner.has_flag(Role::Manager));
        assert!(Role::Owner.has_flag(Role::Admin));
        assert!(Role::Owner.has_flag(Role::Member));
    }

    #[test]
    fn all_returns_union_of_flags() {
        assert_eq!(Permission::all().bits(), 0x000e);
        assert_eq!(AutoAssigned::all().bits(), 0x0007);
    }

    #[test]
    fn accepts_explicit_backing_type() {
        #[flags(u32)]
        enum WideFlags {
            #[flag]
            None,
            #[flag]
            One,
        }

        assert_eq!(::core::mem::size_of::<WideFlags>(), 4);
        assert_eq!(WideFlags::One.bits(), 1u32);
    }

    #[test]
    fn serde_derives_forward() {
        use serde::{Deserialize, Serialize};

        #[flags]
        #[derive(Serialize, Deserialize)]
        enum LocalPermission {
            #[flag]
            None = 0x00,
            #[flag(0x01)]
            Read,
            #[flag(0x02)]
            Write,
        }

        let perm = LocalPermission::Read | LocalPermission::Write;
        let json = ::serde_json::to_string(&perm).unwrap();
        let back: LocalPermission = ::serde_json::from_str(&json).unwrap();

        assert_eq!(back, perm);
    }
}
