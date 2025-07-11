#[derive(Debug)]
pub enum CapabilityKind {
    Thread,
    TranscientPageTable,
    RootPageTable,
    CapTable,
}
