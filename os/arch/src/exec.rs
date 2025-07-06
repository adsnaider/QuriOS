pub trait ExecState: Clone {
    fn save(&self);
    fn dispatch(&self) -> !;
}
