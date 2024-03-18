pub trait PipeOp<T> {
    fn to<F, U>(self, f: F) -> U
    where
        F: FnOnce(T) -> U;

    fn op<F>(self, f: F) -> T
    where
        F: FnOnce(&mut T);
}

impl<T> PipeOp<T> for T {
    fn to<F, U>(self, f: F) -> U
    where
        F: FnOnce(T) -> U,
    {
        f(self)
    }

    fn op<F>(mut self, f: F) -> T
    where
        F: FnOnce(&mut T),
    {
        f(&mut self);
        self
    }
}
