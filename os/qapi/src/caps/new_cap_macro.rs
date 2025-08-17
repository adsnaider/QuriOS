macro_rules! cap_type {
($vis:vis struct $captype:ident;) => {
    use ::derive_more::{From, Into};
    use ::zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};
    use crate::caps::{CapId, CapError};

    #[repr(transparent)]
    #[derive(
        Debug,
        Copy,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        From,
        Into,
        KnownLayout,
        IntoBytes,
        FromBytes,
        Immutable,
    )]
    #[into(CapId, usize)]
    $vis struct $captype(CapId);

    impl $captype {
        pub const fn new(cap: CapId) -> Self {
            Self(cap)
        }

        pub const fn cap(&self) -> CapId {
            self.0
        }
    }

    impl TryFrom<usize> for $captype {
        type Error = CapError;

        fn try_from(value: usize) -> Result<Self, Self::Error> {
            Ok(Self(CapId::try_from(value)?))
        }
    }
};

}
pub(crate) use cap_type as ctype;
