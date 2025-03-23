
pub trait LogSource {
    fn connect(&mut self);
    fn disconnect();
}
