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

use xcb::xproto;

use crate::keyboard::*;


/// Convert a hardware scan code to a key.
///
/// Note: this is a hardcoded layout. We need to detect the user's
/// layout from the system and apply it.
fn code_to_key(code: Code, m: Modifiers) -> KbKey {
    fn a(s: &str) -> KbKey {
        KbKey::Character(s.into())
    }
    fn s(mods: Modifiers, base: &str, shifted: &str) -> KbKey {
        if mods.shift() {
            KbKey::Character(shifted.into())
        } else {
            KbKey::Character(base.into())
        }
    }
    fn n(mods: Modifiers, base: KbKey, num: &str) -> KbKey {
        if mods.contains(Modifiers::NUM_LOCK) != mods.shift() {
            KbKey::Character(num.into())
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

        Code::Escape => KbKey::Escape,
        Code::Backspace => KbKey::Backspace,
        Code::Tab => KbKey::Tab,
        Code::Enter => KbKey::Enter,
        Code::ControlLeft => KbKey::Control,
        Code::ShiftLeft => KbKey::Shift,
        Code::ShiftRight => KbKey::Shift,
        Code::NumpadMultiply => a("*"),
        Code::AltLeft => KbKey::Alt,
        Code::CapsLock => KbKey::CapsLock,
        Code::F1 => KbKey::F1,
        Code::F2 => KbKey::F2,
        Code::F3 => KbKey::F3,
        Code::F4 => KbKey::F4,
        Code::F5 => KbKey::F5,
        Code::F6 => KbKey::F6,
        Code::F7 => KbKey::F7,
        Code::F8 => KbKey::F8,
        Code::F9 => KbKey::F9,
        Code::F10 => KbKey::F10,
        Code::NumLock => KbKey::NumLock,
        Code::ScrollLock => KbKey::ScrollLock,
        Code::Numpad0 => n(m, KbKey::Insert, "0"),
        Code::Numpad1 => n(m, KbKey::End, "1"),
        Code::Numpad2 => n(m, KbKey::ArrowDown, "2"),
        Code::Numpad3 => n(m, KbKey::PageDown, "3"),
        Code::Numpad4 => n(m, KbKey::ArrowLeft, "4"),
        Code::Numpad5 => n(m, KbKey::Clear, "5"),
        Code::Numpad6 => n(m, KbKey::ArrowRight, "6"),
        Code::Numpad7 => n(m, KbKey::Home, "7"),
        Code::Numpad8 => n(m, KbKey::ArrowUp, "8"),
        Code::Numpad9 => n(m, KbKey::PageUp, "9"),
        Code::NumpadSubtract => a("-"),
        Code::NumpadAdd => a("+"),
        Code::NumpadDecimal => n(m, KbKey::Delete, "."),
        Code::IntlBackslash => s(m, "\\", "|"),
        Code::F11 => KbKey::F11,
        Code::F12 => KbKey::F12,
        // This mapping is based on the picture in the w3c spec.
        Code::IntlRo => a("\\"),
        Code::Convert => KbKey::Convert,
        Code::KanaMode => KbKey::KanaMode,
        Code::NonConvert => KbKey::NonConvert,
        Code::NumpadEnter => KbKey::Enter,
        Code::ControlRight => KbKey::Control,
        Code::NumpadDivide => a("/"),
        Code::PrintScreen => KbKey::PrintScreen,
        Code::AltRight => KbKey::Alt,
        Code::Home => KbKey::Home,
        Code::ArrowUp => KbKey::ArrowUp,
        Code::PageUp => KbKey::PageUp,
        Code::ArrowLeft => KbKey::ArrowLeft,
        Code::ArrowRight => KbKey::ArrowRight,
        Code::End => KbKey::End,
        Code::ArrowDown => KbKey::ArrowDown,
        Code::PageDown => KbKey::PageDown,
        Code::Insert => KbKey::Insert,
        Code::Delete => KbKey::Delete,
        Code::AudioVolumeMute => KbKey::AudioVolumeMute,
        Code::AudioVolumeDown => KbKey::AudioVolumeDown,
        Code::AudioVolumeUp => KbKey::AudioVolumeUp,
        Code::NumpadEqual => a("="),
        Code::Pause => KbKey::Pause,
        Code::NumpadComma => a(","),
        Code::Lang1 => KbKey::HangulMode,
        Code::Lang2 => KbKey::HanjaMode,
        Code::IntlYen => a("¥"),
        Code::MetaLeft => KbKey::Meta,
        Code::MetaRight => KbKey::Meta,
        Code::ContextMenu => KbKey::ContextMenu,
        Code::BrowserStop => KbKey::BrowserStop,
        Code::Again => KbKey::Again,
        Code::Props => KbKey::Props,
        Code::Undo => KbKey::Undo,
        Code::Select => KbKey::Select,
        Code::Copy => KbKey::Copy,
        Code::Open => KbKey::Open,
        Code::Paste => KbKey::Paste,
        Code::Find => KbKey::Find,
        Code::Cut => KbKey::Cut,
        Code::Help => KbKey::Help,
        Code::LaunchApp2 => KbKey::LaunchApplication2,
        Code::WakeUp => KbKey::WakeUp,
        Code::LaunchApp1 => KbKey::LaunchApplication1,
        Code::LaunchMail => KbKey::LaunchMail,
        Code::BrowserFavorites => KbKey::BrowserFavorites,
        Code::BrowserBack => KbKey::BrowserBack,
        Code::BrowserForward => KbKey::BrowserForward,
        Code::Eject => KbKey::Eject,
        Code::MediaTrackNext => KbKey::MediaTrackNext,
        Code::MediaPlayPause => KbKey::MediaPlayPause,
        Code::MediaTrackPrevious => KbKey::MediaTrackPrevious,
        Code::MediaStop => KbKey::MediaStop,
        Code::MediaSelect => KbKey::LaunchMediaPlayer,
        Code::BrowserHome => KbKey::BrowserHome,
        Code::BrowserRefresh => KbKey::BrowserRefresh,
        Code::BrowserSearch => KbKey::BrowserSearch,

        _ => KbKey::Unidentified,
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
// `xcb::xproto::ButtonPressEvent`
fn key_mods(mods: u16) -> Modifiers {
    let mut ret = Modifiers::default();
    let mut key_masks = [
        (xproto::MOD_MASK_SHIFT, Modifiers::SHIFT),
        (xproto::MOD_MASK_CONTROL, Modifiers::CONTROL),
        // X11's mod keys are configurable, but this seems
        // like a reasonable default for US keyboards, at least,
        // where the "windows" key seems to be MOD_MASK_4.
        (xproto::MOD_MASK_1, Modifiers::ALT),
        (xproto::MOD_MASK_2, Modifiers::NUM_LOCK),
        (xproto::MOD_MASK_4, Modifiers::META),
        (xproto::MOD_MASK_LOCK, Modifiers::CAPS_LOCK),
    ];
    for (mask, modifiers) in &mut key_masks {
        if mods & (*mask as u16) != 0 {
            ret |= *modifiers;
        }
    }
    ret
}


pub(super) fn convert_key_press_event(
    key_press: &xcb::KeyPressEvent,
) -> KeyEvent {
    let hw_keycode = key_press.detail();
    let code = hardware_keycode_to_code(hw_keycode.into());
    let mods = key_mods(key_press.state());
    let key = code_to_key(code, mods);
    let location = code_to_location(code);
    let state = KeyState::Down;

    KeyEvent {
        code,
        key,
        mods,
        location,
        state,
        repeat: false,
        is_composing: false,
    }
}


pub(super) fn convert_key_release_event(
    key_release: &xcb::KeyReleaseEvent
) -> KeyEvent {
    let hw_keycode = key_release.detail();
    let code = hardware_keycode_to_code(hw_keycode.into());
    let mods = key_mods(key_release.state());
    let key = code_to_key(code, mods);
    let location = code_to_location(code);
    let state = KeyState::Up;

    KeyEvent {
        code,
        key,
        mods,
        location,
        state,
        repeat: false,
        is_composing: false,
    }
}