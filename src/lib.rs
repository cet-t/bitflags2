//! Derive-like bitflags powered by an attribute macro.
//!
//! `bitflags2` lets you declare flags with enum syntax while generating a
//! compact integer-backed flag type. Each variant marked with `#[flag]` becomes
//! an associated constant on the generated type.
//!
//! # Example
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

    #[cfg(feature = "serde")]
    mod serde_tests {
        use crate::tests::Permission;
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        struct Wrapper {
            perm: Permission,
        }

        #[test]
        fn serde_roundtrip() {
            let perm = Permission::Read | Permission::Write;
            let wrapper = Wrapper { perm };

            let json = serde_json::to_string(&wrapper).unwrap();
            let back: Wrapper = serde_json::from_str(&json).unwrap();

            assert_eq!(back.perm, perm);
            assert_eq!(back.perm, Permission::Read | Permission::Write);
        }
    }
}
