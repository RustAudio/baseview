// Copyright 2020 The Druid Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Baseview modifications to druid code:
// - only keep code_to_location function

//! Keyboard types.

#[cfg(any(target_os = "linux", target_os = "macos"))]
use keyboard_types::{Code, Location};

#[cfg(any(target_os = "linux", target_os = "macos"))]
/// Map key code to location.
///
/// The logic for this is adapted from InitKeyEvent in TextInputHandler (in the Mozilla
/// mac port).
///
/// Note: in the original, this is based on kVK constants, but since we don't have those
/// readily available, we use the mapping to code (which should be effectively lossless).
pub fn code_to_location(code: Code) -> Location {
    match code {
        Code::MetaLeft | Code::ShiftLeft | Code::AltLeft | Code::ControlLeft => Location::Left,
        Code::MetaRight | Code::ShiftRight | Code::AltRight | Code::ControlRight => Location::Right,
        Code::Numpad0
        | Code::Numpad1
        | Code::Numpad2
        | Code::Numpad3
        | Code::Numpad4
        | Code::Numpad5
        | Code::Numpad6
        | Code::Numpad7
        | Code::Numpad8
        | Code::Numpad9
        | Code::NumpadAdd
        | Code::NumpadComma
        | Code::NumpadDecimal
        | Code::NumpadDivide
        | Code::NumpadEnter
        | Code::NumpadEqual
        | Code::NumpadMultiply
        | Code::NumpadSubtract => Location::Numpad,
        _ => Location::Standard,
    }
}
