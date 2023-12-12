mod keyboard;
mod view;
mod window;

use cocoa::foundation::NSUInteger;
pub use window::*;

const NSDragOperationNone: NSUInteger = 0;
const NSDragOperationCopy: NSUInteger = 1;
const NSDragOperationLink: NSUInteger = 2;
const NSDragOperationGeneric: NSUInteger = 4;
const NSDragOperationMove: NSUInteger = 16;
