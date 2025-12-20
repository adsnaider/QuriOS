pub mod ctable;
pub mod introspect;
pub mod irq;
pub mod notify;
pub mod retype;
pub mod sync_ipc;
pub mod thread;
pub mod vmtable;

const USIZE_IN_U64: usize = (u64::BITS / usize::BITS) as usize;
const fn u64_to_usize_array(mut value: u64) -> [usize; USIZE_IN_U64] {
    let mut out = [0; USIZE_IN_U64];
    let mut i = 0;
    while i < USIZE_IN_U64 {
        let piece = (value & usize::MAX as u64) as usize;
        out[i] = piece;
        i += 1;
        value = value.unbounded_shl(usize::BITS);
    }
    out
}

const fn usize_array_to_u64(pieces: &[usize]) -> (u64, usize) {
    let mut out: u64 = 0;
    let mut i = 0;
    if pieces.len() < USIZE_IN_U64 {
        panic!("Missing arguments to reconstruct u64");
    }
    while i < USIZE_IN_U64 {
        out = out.unbounded_shr(usize::BITS);
        // SAFETY: Verified the length above.
        out += pieces[i] as u64;
        i += 1;
    }
    (out, USIZE_IN_U64)
}
