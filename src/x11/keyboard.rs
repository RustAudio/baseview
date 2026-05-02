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
// - collect functions from various files
// - update imports, paths etc

//! X11 keyboard handling

use x11rb::protocol::xproto::{KeyButMask, KeyPressEvent, KeyReleaseEvent};

use keyboard_types::*;

use crate::keyboard::code_to_location;

/// Convert a hardware scan code to a key.
///
/// Note: this is a hardcoded layout. We need to detect the user's
/// layout from the system and apply it.
fn code_to_key(code: Code, m: Modifiers) -> Key {
    fn a(s: &str) -> Key {
        Key::Character(s.into())
    }
    fn s(mods: Modifiers, base: &str, shifted: &str) -> Key {
        if mods.contains(Modifiers::SHIFT) {
            Key::Character(shifted.into())
        } else {
            Key::Character(base.into())
        }
    }
    fn n(mods: Modifiers, base: Key, num: &str) -> Key {
        if mods.contains(Modifiers::NUM_LOCK) != mods.contains(Modifiers::SHIFT) {
            Key::Character(num.into())
        } else {
            base
        }
    }
    match code {
        Code::KeyA => s(m, "a", "A"),
        Code::KeyB => s(m, "b", "B"),
        Code::KeyC => s(m, "c", "C"),
        Code::KeyD => s(m, "d", "D"),
        Code::KeyE => s(m, "e", "E"),
        Code::KeyF => s(m, "f", "F"),
        Code::KeyG => s(m, "g", "G"),
        Code::KeyH => s(m, "h", "H"),
        Code::KeyI => s(m, "i", "I"),
        Code::KeyJ => s(m, "j", "J"),
        Code::KeyK => s(m, "k", "K"),
        Code::KeyL => s(m, "l", "L"),
        Code::KeyM => s(m, "m", "M"),
        Code::KeyN => s(m, "n", "N"),
        Code::KeyO => s(m, "o", "O"),
        Code::KeyP => s(m, "p", "P"),
        Code::KeyQ => s(m, "q", "Q"),
        Code::KeyR => s(m, "r", "R"),
        Code::KeyS => s(m, "s", "S"),
        Code::KeyT => s(m, "t", "T"),
        Code::KeyU => s(m, "u", "U"),
        Code::KeyV => s(m, "v", "V"),
        Code::KeyW => s(m, "w", "W"),
        Code::KeyX => s(m, "x", "X"),
        Code::KeyY => s(m, "y", "Y"),
        Code::KeyZ => s(m, "z", "Z"),

        Code::Digit0 => s(m, "0", ")"),
        Code::Digit1 => s(m, "1", "!"),
        Code::Digit2 => s(m, "2", "@"),
        Code::Digit3 => s(m, "3", "#"),
        Code::Digit4 => s(m, "4", "$"),
        Code::Digit5 => s(m, "5", "%"),
        Code::Digit6 => s(m, "6", "^"),
        Code::Digit7 => s(m, "7", "&"),
        Code::Digit8 => s(m, "8", "*"),
        Code::Digit9 => s(m, "9", "("),

        Code::Backquote => s(m, "`", "~"),
        Code::Minus => s(m, "-", "_"),
        Code::Equal => s(m, "=", "+"),
        Code::BracketLeft => s(m, "[", "{"),
        Code::BracketRight => s(m, "]", "}"),
        Code::Backslash => s(m, "\\", "|"),
        Code::Semicolon => s(m, ";", ":"),
        Code::Quote => s(m, "'", "\""),
        Code::Comma => s(m, ",", "<"),
        Code::Period => s(m, ".", ">"),
        Code::Slash => s(m, "/", "?"),

        Code::Space => a(" "),

        Code::Escape => Key::Named(NamedKey::Escape),
        Code::Backspace => Key::Named(NamedKey::Backspace),
        Code::Tab => Key::Named(NamedKey::Tab),
        Code::Enter => Key::Named(NamedKey::Enter),
        Code::ControlLeft => Key::Named(NamedKey::Control),
        Code::ShiftLeft => Key::Named(NamedKey::Shift),
        Code::ShiftRight => Key::Named(NamedKey::Shift),
        Code::NumpadMultiply => a("*"),
        Code::AltLeft => Key::Named(NamedKey::Alt),
        Code::CapsLock => Key::Named(NamedKey::CapsLock),
        Code::F1 => Key::Named(NamedKey::F1),
        Code::F2 => Key::Named(NamedKey::F2),
        Code::F3 => Key::Named(NamedKey::F3),
        Code::F4 => Key::Named(NamedKey::F4),
        Code::F5 => Key::Named(NamedKey::F5),
        Code::F6 => Key::Named(NamedKey::F6),
        Code::F7 => Key::Named(NamedKey::F7),
        Code::F8 => Key::Named(NamedKey::F8),
        Code::F9 => Key::Named(NamedKey::F9),
        Code::F10 => Key::Named(NamedKey::F10),
        Code::NumLock => Key::Named(NamedKey::NumLock),
        Code::ScrollLock => Key::Named(NamedKey::ScrollLock),
        Code::Numpad0 => n(m, Key::Named(NamedKey::Insert), "0"),
        Code::Numpad1 => n(m, Key::Named(NamedKey::End), "1"),
        Code::Numpad2 => n(m, Key::Named(NamedKey::ArrowDown), "2"),
        Code::Numpad3 => n(m, Key::Named(NamedKey::PageDown), "3"),
        Code::Numpad4 => n(m, Key::Named(NamedKey::ArrowLeft), "4"),
        Code::Numpad5 => n(m, Key::Named(NamedKey::Clear), "5"),
        Code::Numpad6 => n(m, Key::Named(NamedKey::ArrowRight), "6"),
        Code::Numpad7 => n(m, Key::Named(NamedKey::Home), "7"),
        Code::Numpad8 => n(m, Key::Named(NamedKey::ArrowUp), "8"),
        Code::Numpad9 => n(m, Key::Named(NamedKey::PageUp), "9"),
        Code::NumpadSubtract => a("-"),
        Code::NumpadAdd => a("+"),
        Code::NumpadDecimal => n(m, Key::Delete, "."),
        Code::IntlBackslash => s(m, "\\", "|"),
        Code::F11 => Key::Named(NamedKey::F11),
        Code::F12 => Key::Named(NamedKey::F12),
        // This mapping is based on the picture in the w3c spec.
        Code::IntlRo => a("\\"),
        Code::Convert => Key::Named(NamedKey::Convert),
        Code::KanaMode => Key::Named(NamedKey::KanaMode),
        Code::NonConvert => Key::Named(NamedKey::NonConvert),
        Code::NumpadEnter => Key::Named(NamedKey::Enter),
        Code::ControlRight => Key::Named(NamedKey::Control),
        Code::NumpadDivide => a("/"),
        Code::PrintScreen => Key::Named(NamedKey::PrintScreen),
        Code::AltRight => Key::Named(NamedKey::Alt),
        Code::Home => Key::Named(NamedKey::Home),
        Code::ArrowUp => Key::Named(NamedKey::ArrowUp),
        Code::PageUp => Key::Named(NamedKey::PageUp),
        Code::ArrowLeft => Key::Named(NamedKey::ArrowLeft),
        Code::ArrowRight => Key::Named(NamedKey::ArrowRight),
        Code::End => Key::Named(NamedKey::End),
        Code::ArrowDown => Key::Named(NamedKey::ArrowDown),
        Code::PageDown => Key::Named(NamedKey::PageDown),
        Code::Insert => Key::Named(NamedKey::Insert),
        Code::Delete => Key::Named(NamedKey::Delete),
        Code::AudioVolumeMute => Key::Named(NamedKey::AudioVolumeMute),
        Code::AudioVolumeDown => Key::Named(NamedKey::AudioVolumeDown),
        Code::AudioVolumeUp => Key::Named(NamedKey::AudioVolumeUp),
        Code::NumpadEqual => a("="),
        Code::Pause => Key::Named(NamedKey::Pause),
        Code::NumpadComma => a(","),
        Code::Lang1 => Key::Named(NamedKey::HangulMode),
        Code::Lang2 => Key::Named(NamedKey::HanjaMode),
        Code::IntlYen => a("¥"),
        Code::MetaLeft => Key::Named(NamedKey::Meta),
        Code::MetaRight => Key::Named(NamedKey::Meta),
        Code::ContextMenu => Key::Named(NamedKey::ContextMenu),
        Code::BrowserStop => Key::Named(NamedKey::BrowserStop),
        Code::Again => Key::Named(NamedKey::Again),
        Code::Props => Key::Named(NamedKey::Props),
        Code::Undo => Key::Named(NamedKey::Undo),
        Code::Select => Key::Named(NamedKey::Select),
        Code::Copy => Key::Named(NamedKey::Copy),
        Code::Open => Key::Named(NamedKey::Open),
        Code::Paste => Key::Named(NamedKey::Paste),
        Code::Find => Key::Named(NamedKey::Find),
        Code::Cut => Key::Named(NamedKey::Cut),
        Code::Help => Key::Named(NamedKey::Help),
        Code::LaunchApp2 => Key::Named(NamedKey::LaunchApplication2),
        Code::WakeUp => Key::Named(NamedKey::WakeUp),
        Code::LaunchApp1 => Key::Named(NamedKey::LaunchApplication1),
        Code::LaunchMail => Key::Named(NamedKey::LaunchMail),
        Code::BrowserFavorites => Key::Named(NamedKey::BrowserFavorites),
        Code::BrowserBack => Key::Named(NamedKey::BrowserBack),
        Code::BrowserForward => Key::Named(NamedKey::BrowserForward),
        Code::Eject => Key::Named(NamedKey::Eject),
        Code::MediaTrackNext => Key::Named(NamedKey::MediaTrackNext),
        Code::MediaPlayPause => Key::Named(NamedKey::MediaPlayPause),
        Code::MediaTrackPrevious => Key::Named(NamedKey::MediaTrackPrevious),
        Code::MediaStop => Key::Named(NamedKey::MediaStop),
        Code::MediaSelect => Key::Named(NamedKey::LaunchMediaPlayer),
        Code::BrowserHome => Key::Named(NamedKey::BrowserHome),
        Code::BrowserRefresh => Key::Named(NamedKey::BrowserRefresh),
        Code::BrowserSearch => Key::Named(NamedKey::BrowserSearch),

        _ => Key::Named(NamedKey::Unidentified),
    }
}

#[cfg(target_os = "linux")]
/// Map hardware keycode to code.
///
/// In theory, the hardware keycode is device dependent, but in
/// practice it's probably pretty reliable.
///
/// The logic is based on NativeKeyToDOMCodeName.h in Mozilla.
fn hardware_keycode_to_code(hw_keycode: u16) -> Code {
    match hw_keycode {
        0x0009 => Code::Escape,
        0x000A => Code::Digit1,
        0x000B => Code::Digit2,
        0x000C => Code::Digit3,
        0x000D => Code::Digit4,
        0x000E => Code::Digit5,
        0x000F => Code::Digit6,
        0x0010 => Code::Digit7,
        0x0011 => Code::Digit8,
        0x0012 => Code::Digit9,
        0x0013 => Code::Digit0,
        0x0014 => Code::Minus,
        0x0015 => Code::Equal,
        0x0016 => Code::Backspace,
        0x0017 => Code::Tab,
        0x0018 => Code::KeyQ,
        0x0019 => Code::KeyW,
        0x001A => Code::KeyE,
        0x001B => Code::KeyR,
        0x001C => Code::KeyT,
        0x001D => Code::KeyY,
        0x001E => Code::KeyU,
        0x001F => Code::KeyI,
        0x0020 => Code::KeyO,
        0x0021 => Code::KeyP,
        0x0022 => Code::BracketLeft,
        0x0023 => Code::BracketRight,
        0x0024 => Code::Enter,
        0x0025 => Code::ControlLeft,
        0x0026 => Code::KeyA,
        0x0027 => Code::KeyS,
        0x0028 => Code::KeyD,
        0x0029 => Code::KeyF,
        0x002A => Code::KeyG,
        0x002B => Code::KeyH,
        0x002C => Code::KeyJ,
        0x002D => Code::KeyK,
        0x002E => Code::KeyL,
        0x002F => Code::Semicolon,
        0x0030 => Code::Quote,
        0x0031 => Code::Backquote,
        0x0032 => Code::ShiftLeft,
        0x0033 => Code::Backslash,
        0x0034 => Code::KeyZ,
        0x0035 => Code::KeyX,
        0x0036 => Code::KeyC,
        0x0037 => Code::KeyV,
        0x0038 => Code::KeyB,
        0x0039 => Code::KeyN,
        0x003A => Code::KeyM,
        0x003B => Code::Comma,
        0x003C => Code::Period,
        0x003D => Code::Slash,
        0x003E => Code::ShiftRight,
        0x003F => Code::NumpadMultiply,
        0x0040 => Code::AltLeft,
        0x0041 => Code::Space,
        0x0042 => Code::CapsLock,
        0x0043 => Code::F1,
        0x0044 => Code::F2,
        0x0045 => Code::F3,
        0x0046 => Code::F4,
        0x0047 => Code::F5,
        0x0048 => Code::F6,
        0x0049 => Code::F7,
        0x004A => Code::F8,
        0x004B => Code::F9,
        0x004C => Code::F10,
        0x004D => Code::NumLock,
        0x004E => Code::ScrollLock,
        0x004F => Code::Numpad7,
        0x0050 => Code::Numpad8,
        0x0051 => Code::Numpad9,
        0x0052 => Code::NumpadSubtract,
        0x0053 => Code::Numpad4,
        0x0054 => Code::Numpad5,
        0x0055 => Code::Numpad6,
        0x0056 => Code::NumpadAdd,
        0x0057 => Code::Numpad1,
        0x0058 => Code::Numpad2,
        0x0059 => Code::Numpad3,
        0x005A => Code::Numpad0,
        0x005B => Code::NumpadDecimal,
        0x005E => Code::IntlBackslash,
        0x005F => Code::F11,
        0x0060 => Code::F12,
        0x0061 => Code::IntlRo,
        0x0064 => Code::Convert,
        0x0065 => Code::KanaMode,
        0x0066 => Code::NonConvert,
        0x0068 => Code::NumpadEnter,
        0x0069 => Code::ControlRight,
        0x006A => Code::NumpadDivide,
        0x006B => Code::PrintScreen,
        0x006C => Code::AltRight,
        0x006E => Code::Home,
        0x006F => Code::ArrowUp,
        0x0070 => Code::PageUp,
        0x0071 => Code::ArrowLeft,
        0x0072 => Code::ArrowRight,
        0x0073 => Code::End,
        0x0074 => Code::ArrowDown,
        0x0075 => Code::PageDown,
        0x0076 => Code::Insert,
        0x0077 => Code::Delete,
        0x0079 => Code::AudioVolumeMute,
        0x007A => Code::AudioVolumeDown,
        0x007B => Code::AudioVolumeUp,
        0x007D => Code::NumpadEqual,
        0x007F => Code::Pause,
        0x0081 => Code::NumpadComma,
        0x0082 => Code::Lang1,
        0x0083 => Code::Lang2,
        0x0084 => Code::IntlYen,
        0x0085 => Code::MetaLeft,
        0x0086 => Code::MetaRight,
        0x0087 => Code::ContextMenu,
        0x0088 => Code::BrowserStop,
        0x0089 => Code::Again,
        0x008A => Code::Props,
        0x008B => Code::Undo,
        0x008C => Code::Select,
        0x008D => Code::Copy,
        0x008E => Code::Open,
        0x008F => Code::Paste,
        0x0090 => Code::Find,
        0x0091 => Code::Cut,
        0x0092 => Code::Help,
        0x0094 => Code::LaunchApp2,
        0x0097 => Code::WakeUp,
        0x0098 => Code::LaunchApp1,
        // key to right of volume controls on T430s produces 0x9C
        // but no documentation of what it should map to :/
        0x00A3 => Code::LaunchMail,
        0x00A4 => Code::BrowserFavorites,
        0x00A6 => Code::BrowserBack,
        0x00A7 => Code::BrowserForward,
        0x00A9 => Code::Eject,
        0x00AB => Code::MediaTrackNext,
        0x00AC => Code::MediaPlayPause,
        0x00AD => Code::MediaTrackPrevious,
        0x00AE => Code::MediaStop,
        0x00B3 => Code::MediaSelect,
        0x00B4 => Code::BrowserHome,
        0x00B5 => Code::BrowserRefresh,
        0x00E1 => Code::BrowserSearch,
        _ => Code::Unidentified,
    }
}

// Extracts the keyboard modifiers from, e.g., the `state` field of
// `x11rb::protocol::xproto::ButtonPressEvent`
pub(super) fn key_mods(mods: KeyButMask) -> Modifiers {
    let mut ret = Modifiers::default();
    let key_masks = [
        (KeyButMask::SHIFT, Modifiers::SHIFT),
        (KeyButMask::CONTROL, Modifiers::CONTROL),
        // X11's mod keys are configurable, but this seems
        // like a reasonable default for US keyboards, at least,
        // where the "windows" key seems to be MOD_MASK_4.
        (KeyButMask::MOD1, Modifiers::ALT),
        (KeyButMask::MOD2, Modifiers::NUM_LOCK),
        (KeyButMask::MOD4, Modifiers::META),
        (KeyButMask::LOCK, Modifiers::CAPS_LOCK),
    ];
    for (mask, modifiers) in &key_masks {
        if mods.contains(*mask) {
            ret |= *modifiers;
        }
    }
    ret
}

pub(super) fn convert_key_press_event(key_press: &KeyPressEvent) -> KeyboardEvent {
    let hw_keycode = key_press.detail;
    let code = hardware_keycode_to_code(hw_keycode.into());
    let modifiers = key_mods(key_press.state);
    let key = code_to_key(code, modifiers);
    let location = code_to_location(code);
    let state = KeyState::Down;

    KeyboardEvent { code, key, modifiers, location, state, repeat: false, is_composing: false }
}

pub(super) fn convert_key_release_event(key_release: &KeyReleaseEvent) -> KeyboardEvent {
    let hw_keycode = key_release.detail;
    let code = hardware_keycode_to_code(hw_keycode.into());
    let modifiers = key_mods(key_release.state);
    let key = code_to_key(code, modifiers);
    let location = code_to_location(code);
    let state = KeyState::Up;

    KeyboardEvent { code, key, modifiers, location, state, repeat: false, is_composing: false }
}
