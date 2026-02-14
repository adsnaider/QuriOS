use crate::sysops::Introspect;

pub struct Shadow<I: Introspect> {
    opaque: I,
}
