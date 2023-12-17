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
// - move from_nsstring function to this file
// - update imports, paths etc

//! Conversion of platform keyboard event into cross-platform event.

use std::cell::Cell;

use cocoa::appkit::{NSEvent, NSEventModifierFlags, NSEventType};
use cocoa::base::id;
use cocoa::foundation::NSString;
use keyboard_types::{Code, Key, KeyState, KeyboardEvent, Modifiers};
use objc::{msg_send, sel, sel_impl};

use crate::keyboard::code_to_location;

pub(crate) fn from_nsstring(s: id) -> String {
    unsafe {
        let slice = std::slice::from_raw_parts(s.UTF8String() as *const _, s.len());
        let result = std::str::from_utf8_unchecked(slice);
        result.into()
    }
}

/// State for processing of keyboard events.
///
/// This needs to be stateful for proper processing of dead keys. The current
/// implementation is somewhat primitive and is not based on IME; in the future
/// when IME is implemented, it will need to be redone somewhat, letting the IME
/// be the authoritative source of truth for Unicode string values of keys.
///
/// Most of the logic in this module is adapted from Mozilla, and in particular
/// TextInputHandler.mm.
pub(crate) struct KeyboardState {
    last_mods: Cell<NSEventModifierFlags>,
}

/// Convert a macOS platform key code (keyCode field of NSEvent).
///
/// The primary source for this mapping is:
/// https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/code/code_values
///
/// It should also match up with CODE_MAP_MAC bindings in
/// NativeKeyToDOMCodeName.h.
fn key_code_to_code(key_code: u16) -> Code {
    match key_code {
        0x00 => Code::KeyA,
        0x01 => Code::KeyS,
        0x02 => Code::KeyD,
        0x03 => Code::KeyF,
        0x04 => Code::KeyH,
        0x05 => Code::KeyG,
        0x06 => Code::KeyZ,
        0x07 => Code::KeyX,
        0x08 => Code::KeyC,
        0x09 => Code::KeyV,
        0x0a => Code::IntlBackslash,
        0x0b => Code::KeyB,
        0x0c => Code::KeyQ,
        0x0d => Code::KeyW,
        0x0e => Code::KeyE,
        0x0f => Code::KeyR,
        0x10 => Code::KeyY,
        0x11 => Code::KeyT,
        0x12 => Code::Digit1,
        0x13 => Code::Digit2,
        0x14 => Code::Digit3,
        0x15 => Code::Digit4,
        0x16 => Code::Digit6,
        0x17 => Code::Digit5,
        0x18 => Code::Equal,
        0x19 => Code::Digit9,
        0x1a => Code::Digit7,
        0x1b => Code::Minus,
        0x1c => Code::Digit8,
        0x1d => Code::Digit0,
        0x1e => Code::BracketRight,
        0x1f => Code::KeyO,
        0x20 => Code::KeyU,
        0x21 => Code::BracketLeft,
        0x22 => Code::KeyI,
        0x23 => Code::KeyP,
        0x24 => Code::Enter,
        0x25 => Code::KeyL,
        0x26 => Code::KeyJ,
        0x27 => Code::Quote,
        0x28 => Code::KeyK,
        0x29 => Code::Semicolon,
        0x2a => Code::Backslash,
        0x2b => Code::Comma,
        0x2c => Code::Slash,
        0x2d => Code::KeyN,
        0x2e => Code::KeyM,
        0x2f => Code::Period,
        0x30 => Code::Tab,
        0x31 => Code::Space,
        0x32 => Code::Backquote,
        0x33 => Code::Backspace,
        0x34 => Code::NumpadEnter,
        0x35 => Code::Escape,
        0x36 => Code::MetaRight,
        0x37 => Code::MetaLeft,
        0x38 => Code::ShiftLeft,
        0x39 => Code::CapsLock,
        // Note: in the linked source doc, this is "OSLeft"
        0x3a => Code::AltLeft,
        0x3b => Code::ControlLeft,
        0x3c => Code::ShiftRight,
        // Note: in the linked source doc, this is "OSRight"
        0x3d => Code::AltRight,
        0x3e => Code::ControlRight,
        0x3f => Code::Fn, // No events fired
        //0x40 => Code::F17,
        0x41 => Code::NumpadDecimal,
        0x43 => Code::NumpadMultiply,
        0x45 => Code::NumpadAdd,
        0x47 => Code::NumLock,
        0x48 => Code::AudioVolumeUp,
        0x49 => Code::AudioVolumeDown,
        0x4a => Code::AudioVolumeMute,
        0x4b => Code::NumpadDivide,
        0x4c => Code::NumpadEnter,
        0x4e => Code::NumpadSubtract,
        //0x4f => Code::F18,
        //0x50 => Code::F19,
        0x51 => Code::NumpadEqual,
        0x52 => Code::Numpad0,
        0x53 => Code::Numpad1,
        0x54 => Code::Numpad2,
        0x55 => Code::Numpad3,
        0x56 => Code::Numpad4,
        0x57 => Code::Numpad5,
        0x58 => Code::Numpad6,
        0x59 => Code::Numpad7,
        //0x5a => Code::F20,
        0x5b => Code::Numpad8,
        0x5c => Code::Numpad9,
        0x5d => Code::IntlYen,
        0x5e => Code::IntlRo,
        0x5f => Code::NumpadComma,
        0x60 => Code::F5,
        0x61 => Code::F6,
        0x62 => Code::F7,
        0x63 => Code::F3,
        0x64 => Code::F8,
        0x65 => Code::F9,
        0x66 => Code::Lang2,
        0x67 => Code::F11,
        0x68 => Code::Lang1,
        // Note: this is listed as F13, but in testing with a standard
        // USB kb, this the code produced by PrtSc.
        0x69 => Code::PrintScreen,
        //0x6a => Code::F16,
        //0x6b => Code::F14,
        0x6d => Code::F10,
        0x6e => Code::ContextMenu,
        0x6f => Code::F12,
        //0x71 => Code::F15,
        0x72 => Code::Help,
        0x73 => Code::Home,
        0x74 => Code::PageUp,
        0x75 => Code::Delete,
        0x76 => Code::F4,
        0x77 => Code::End,
        0x78 => Code::F2,
        0x79 => Code::PageDown,
        0x7a => Code::F1,
        0x7b => Code::ArrowLeft,
        0x7c => Code::ArrowRight,
        0x7d => Code::ArrowDown,
        0x7e => Code::ArrowUp,
        _ => Code::Unidentified,
    }
}

/// Convert code to key.
///
/// On macOS, for non-printable keys, the keyCode we get from the event serves is
/// really more of a key than a physical scan code.
///
/// When this function returns None, the code can be considered printable.
///
/// The logic for this function is derived from KEY_MAP_COCOA bindings in
/// NativeKeyToDOMKeyName.h.
fn code_to_key(code: Code) -> Option<Key> {
    Some(match code {
        Code::Escape => Key::Escape,
        Code::ShiftLeft | Code::ShiftRight => Key::Shift,
        Code::AltLeft | Code::AltRight => Key::Alt,
        Code::MetaLeft | Code::MetaRight => Key::Meta,
        Code::ControlLeft | Code::ControlRight => Key::Control,
        Code::CapsLock => Key::CapsLock,
        // kVK_ANSI_KeypadClear
        Code::NumLock => Key::Clear,
        Code::Fn => Key::Fn,
        Code::F1 => Key::F1,
        Code::F2 => Key::F2,
        Code::F3 => Key::F3,
        Code::F4 => Key::F4,
        Code::F5 => Key::F5,
        Code::F6 => Key::F6,
        Code::F7 => Key::F7,
        Code::F8 => Key::F8,
        Code::F9 => Key::F9,
        Code::F10 => Key::F10,
        Code::F11 => Key::F11,
        Code::F12 => Key::F12,
        Code::Pause => Key::Pause,
        Code::ScrollLock => Key::ScrollLock,
        Code::PrintScreen => Key::PrintScreen,
        Code::Insert => Key::Insert,
        Code::Delete => Key::Delete,
        Code::Tab => Key::Tab,
        Code::Backspace => Key::Backspace,
        Code::ContextMenu => Key::ContextMenu,
        // kVK_JIS_Kana
        Code::Lang1 => Key::KanjiMode,
        // kVK_JIS_Eisu
        Code::Lang2 => Key::Eisu,
        Code::Home => Key::Home,
        Code::End => Key::End,
        Code::PageUp => Key::PageUp,
        Code::PageDown => Key::PageDown,
        Code::ArrowLeft => Key::ArrowLeft,
        Code::ArrowRight => Key::ArrowRight,
        Code::ArrowUp => Key::ArrowUp,
        Code::ArrowDown => Key::ArrowDown,
        Code::Enter => Key::Enter,
        Code::NumpadEnter => Key::Enter,
        Code::Help => Key::Help,
        _ => return None,
    })
}

fn is_valid_key(s: &str) -> bool {
    match s.chars().next() {
        None => false,
        Some(c) => c >= ' ' && c != '\x7f' && !('\u{e000}'..'\u{f900}').contains(&c),
    }
}

fn is_modifier_code(code: Code) -> bool {
    matches!(
        code,
        Code::ShiftLeft
            | Code::ShiftRight
            | Code::AltLeft
            | Code::AltRight
            | Code::ControlLeft
            | Code::ControlRight
            | Code::MetaLeft
            | Code::MetaRight
            | Code::CapsLock
            | Code::Help
    )
}

impl KeyboardState {
    pub(crate) fn new() -> KeyboardState {
        let last_mods = Cell::new(NSEventModifierFlags::empty());
        KeyboardState { last_mods }
    }

    pub(crate) fn last_mods(&self) -> NSEventModifierFlags {
        self.last_mods.get()
    }

    pub(crate) fn process_native_event(&self, event: id) -> Option<KeyboardEvent> {
        unsafe {
            let event_type = event.eventType();
            let key_code = event.keyCode();
            let code = key_code_to_code(key_code);
            let location = code_to_location(code);
            let raw_mods = event.modifierFlags();
            let modifiers = make_modifiers(raw_mods);
            let state = match event_type {
                NSEventType::NSKeyDown => KeyState::Down,
                NSEventType::NSKeyUp => KeyState::Up,
                NSEventType::NSFlagsChanged => {
                    // We use `bits` here because we want to distinguish the
                    // device dependent bits (when both left and right keys
                    // may be pressed, for example).
                    let any_down = raw_mods.bits() & !self.last_mods.get().bits();
                    self.last_mods.set(raw_mods);
                    if is_modifier_code(code) {
                        if any_down == 0 {
                            KeyState::Up
                        } else {
                            KeyState::Down
                        }
                    } else {
                        // HandleFlagsChanged has some logic for this; it might
                        // happen when an app is deactivated by Command-Tab. In
                        // that case, the best thing to do is synthesize the event
                        // from the modifiers. But a challenge there is that we
                        // might get multiple events.
                        return None;
                    }
                }
                _ => unreachable!(),
            };
            let is_composing = false;
            let repeat: bool = event_type == NSEventType::NSKeyDown && msg_send![event, isARepeat];
            let key = if let Some(key) = code_to_key(code) {
                key
            } else {
                let characters = from_nsstring(event.characters());
                if is_valid_key(&characters) {
                    Key::Character(characters)
                } else {
                    let chars_ignoring = from_nsstring(event.charactersIgnoringModifiers());
                    if is_valid_key(&chars_ignoring) {
                        Key::Character(chars_ignoring)
                    } else {
                        // There may be more heroic things we can do here.
                        Key::Unidentified
                    }
                }
            };
            let event =
                KeyboardEvent { code, key, location, modifiers, state, is_composing, repeat };
            Some(event)
        }
    }
}

const MODIFIER_MAP: &[(NSEventModifierFlags, Modifiers)] = &[
    (NSEventModifierFlags::NSShiftKeyMask, Modifiers::SHIFT),
    (NSEventModifierFlags::NSAlternateKeyMask, Modifiers::ALT),
    (NSEventModifierFlags::NSControlKeyMask, Modifiers::CONTROL),
    (NSEventModifierFlags::NSCommandKeyMask, Modifiers::META),
    (NSEventModifierFlags::NSAlphaShiftKeyMask, Modifiers::CAPS_LOCK),
];

pub(crate) fn make_modifiers(raw: NSEventModifierFlags) -> Modifiers {
    let mut modifiers = Modifiers::empty();
    for &(flags, mods) in MODIFIER_MAP {
        if raw.contains(flags) {
            modifiers |= mods;
        }
    }
    modifiers
}
