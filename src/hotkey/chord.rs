use rdev::Key;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub enum HotkeyAction {
    // Static mode — fixed addressed slots 1-9
    StaticCopy(usize),
    StaticPaste(usize),
    StaticCut(usize),
    StaticNavigateNext,
    StaticNavigatePrev,
    StaticDeleteCurrent,

    // Dynamic mode — recency ring
    DynamicCopy,
    DynamicCut,
    DynamicPaste,
    DynamicNavigateNext,
    DynamicNavigatePrev,
    DynamicDeleteCurrent,

    // Internal
    CursorReset,
}

pub struct ChordDetector {
    held:         HashSet<Key>,
    tab_consumed: bool,

    /// The digit pressed while Cmd+Opt was active.
    /// Survives the digit key being released so C/V/X can read it.
    /// Cleared only when Cmd or Opt is released (chord broken).
    last_digit: Option<usize>,

    /// Set to true when a key (C/V/X) is pressed while Cmd+Opt active.
    /// Used at release time so we don't miss actions if Opt is released
    /// slightly before the letter key.
    keys_pressed_in_chord: HashSet<Key>,
}

impl ChordDetector {
    pub fn new() -> Self {
        Self {
            held:                  HashSet::new(),
            tab_consumed:          false,
            last_digit:            None,
            keys_pressed_in_chord: HashSet::new(),
        }
    }

    pub fn key_down(&mut self, key: Key) {
        // If Cmd+Opt active when this key goes down, remember it
        if self.cmd() && self.opt() {
            if let Some(d) = key_to_digit(&key) {
                self.last_digit = Some(d);
            }
            // Track that this key was pressed inside the chord
            self.keys_pressed_in_chord.insert(key.clone());
        }
        self.held.insert(key);
    }

    pub fn key_up(&mut self, key: Key) {
        if key == Key::Tab {
            self.tab_consumed = false;
        }

        // Chord broken when Cmd or Opt released — clear chord state,
        // but retain keys in keys_pressed_in_chord that are still being held
        // so that they can trigger timing-safely on release.
        if key == Key::MetaLeft || key == Key::MetaRight
            || key == Key::Alt   || key == Key::AltGr
        {
            self.last_digit = None;
            self.keys_pressed_in_chord.retain(|k| self.held.contains(k));
        }

        self.held.remove(&key);
    }

    // ── Modifier state ────────────────────────────────────────────────

    pub fn cmd(&self) -> bool {
        self.held.contains(&Key::MetaLeft)
            || self.held.contains(&Key::MetaRight)
    }

    pub fn opt(&self) -> bool {
        self.held.contains(&Key::Alt)
            || self.held.contains(&Key::AltGr)
    }

    fn shift(&self) -> bool {
        self.held.contains(&Key::ShiftLeft)
            || self.held.contains(&Key::ShiftRight)
    }

    // ── Press-triggered: Tab / Esc ────────────────────────────────────

    pub fn evaluate_press(
        &mut self,
        key: &Key,
        mode_static: bool,
    ) -> Option<HotkeyAction> {
        if !self.cmd() || !self.opt() {
            return None;
        }

        match key {
            Key::Tab => {
                if self.tab_consumed { return None; }
                self.tab_consumed = true;

                if self.held.contains(&Key::Escape) {
                    return Some(if mode_static {
                        HotkeyAction::StaticDeleteCurrent
                    } else {
                        HotkeyAction::DynamicDeleteCurrent
                    });
                }

                if self.shift() {
                    Some(if mode_static {
                        HotkeyAction::StaticNavigatePrev
                    } else {
                        HotkeyAction::DynamicNavigatePrev
                    })
                } else {
                    Some(if mode_static {
                        HotkeyAction::StaticNavigateNext
                    } else {
                        HotkeyAction::DynamicNavigateNext
                    })
                }
            }

            Key::Escape if self.held.contains(&Key::Tab) => {
                Some(if mode_static {
                    HotkeyAction::StaticDeleteCurrent
                } else {
                    HotkeyAction::DynamicDeleteCurrent
                })
            }

            _ => None,
        }
    }

    // ── Release-triggered: C / V / X ─────────────────────────────────
    // Fires when the key is released IF it was pressed while chord was active.
    // Does NOT require Cmd+Opt to still be held at release time —
    // this handles the common case where a modifier is released
    // a few ms before the letter key.

    pub fn evaluate_release(
        &mut self,
        key: &Key,
        mode_static: bool,
    ) -> Option<HotkeyAction> {
        // The key must have been pressed inside the chord
        if !self.keys_pressed_in_chord.contains(key) {
            return None;
        }

        // Consume so it can't fire twice
        self.keys_pressed_in_chord.remove(key);

        match key {
            Key::KeyC => {
                if mode_static {
                    // Slot required — no digit means no-op
                    self.last_digit.map(HotkeyAction::StaticCopy)
                } else {
                    Some(HotkeyAction::DynamicCopy)
                }
            }

            Key::KeyX => {
                if mode_static {
                    self.last_digit.map(HotkeyAction::StaticCut)
                } else {
                    Some(HotkeyAction::DynamicCut)
                }
            }

            Key::KeyV => {
                if mode_static {
                    self.last_digit.map(HotkeyAction::StaticPaste)
                } else {
                    Some(HotkeyAction::DynamicPaste)
                }
            }

            _ => None,
        }
    }
}

fn key_to_digit(key: &Key) -> Option<usize> {
    match key {
        Key::Num1 => Some(1), Key::Num2 => Some(2), Key::Num3 => Some(3),
        Key::Num4 => Some(4), Key::Num5 => Some(5), Key::Num6 => Some(6),
        Key::Num7 => Some(7), Key::Num8 => Some(8), Key::Num9 => Some(9),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timing_safe_release_triggering() {
        let mut detector = ChordDetector::new();

        // 1. User presses MetaLeft (Cmd) and Alt (Opt)
        detector.key_down(Key::MetaLeft);
        detector.key_down(Key::Alt);

        // 2. User presses KeyC (Copy)
        detector.key_down(Key::KeyC);

        // 3. User releases Alt (Opt) slightly before KeyC
        detector.key_up(Key::Alt);

        // 4. User releases KeyC (Copy)
        let action = detector.evaluate_release(&Key::KeyC, false);

        // 5. Action should be detected as DynamicCopy!
        assert_eq!(action, Some(HotkeyAction::DynamicCopy));
    }
}