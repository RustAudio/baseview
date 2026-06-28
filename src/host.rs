use crate::WindowSize;

pub trait HostHandler: 'static {
    fn request_resize(&mut self, size: WindowSize);
    fn destroyed(&mut self);
}
