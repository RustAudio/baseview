// This is required because the objc crate is causing a lot of warnings: https://github.com/SSheldon/rust-objc/issues/125
// Eventually we should migrate to the objc2 crate and remove this.
#![allow(unexpected_cfgs)]

mod cursor;
mod keyboard;
mod view;
mod window;

pub use window::*;

#[allow(non_upper_case_globals)]
mod consts {
    use cocoa::foundation::NSUInteger;

    pub const NSDragOperationNone: NSUInteger = 0;
    pub const NSDragOperationCopy: NSUInteger = 1;
    pub const NSDragOperationLink: NSUInteger = 2;
    pub const NSDragOperationGeneric: NSUInteger = 4;
    pub const NSDragOperationMove: NSUInteger = 16;
}
use consts::*;
