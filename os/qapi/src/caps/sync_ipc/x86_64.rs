#[derive(Debug, Copy, Clone)]
pub struct ExceptionAbi;

#[derive(Debug)]
pub struct ExceptionRetAbi;

impl SyncAbi for ExceptionAbi {
    type Args = (ExceptionKind, Option<u64>);
    type Ret = PositiveIsize;
}
