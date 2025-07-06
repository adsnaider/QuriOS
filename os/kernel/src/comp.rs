use arch::mem::Addrspace;

#[derive(Debug)]
pub struct Component<A> {
    _addrspace: A,
}
impl<A> Component<A> {
    pub fn active_addrspace(&self)
    where
        A: Addrspace,
    {
        todo!()
    }
}
