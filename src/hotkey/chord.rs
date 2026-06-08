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
    DynamicPlainPaste,
    DynamicNavigateNext,
    DynamicNavigatePrev,
    DynamicDeleteCurrent,

    // Static mode — plain paste variant
    StaticPlainPaste(usize),

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

        // Chord broken when Cmd or Opt released — retain any keys still held
        // so they can trigger timing-safely on release.  Also retain last_digit
        // while pending chord keys exist: static mode needs it in evaluate_release().
        if key == Key::MetaLeft || key == Key::MetaRight
            || key == Key::Alt   || key == Key::AltGr
        {
            self.keys_pressed_in_chord.retain(|k| self.held.contains(k));
            // Only clear last_digit when no pending chord keys remain.
            // If keys are still pending, evaluate_release() needs it for
            // StaticCopy / StaticCut / StaticPaste.
            if self.keys_pressed_in_chord.is_empty() {
                self.last_digit = None;
            }
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

        let action = match key {
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

            Key::KeyB => {
                if mode_static {
                    self.last_digit.map(HotkeyAction::StaticPlainPaste)
                } else {
                    Some(HotkeyAction::DynamicPlainPaste)
                }
            }

            _ => None,
        };

        // Now that we've consumed the pending key, clear last_digit if
        // no further chord keys remain (avoids stale digit on next chord).
        if self.keys_pressed_in_chord.is_empty() {
            self.last_digit = None;
        }

        action
    }

    /// Returns true if the key was pressed inside the chord and is still pending.
    /// Used by handle_event in grabber.rs to decide whether to call evaluate_release
    /// even when the modifier was already released.
    pub fn is_pending_chord_key(&self, key: &Key) -> bool {
        self.keys_pressed_in_chord.contains(key)
    }

    /// Reset the detector to a clean Idle state, dropping all held keys, any
    /// pending chord keys, the addressed digit, and the Tab latch.
    ///
    /// Ownership (issue #27, review pt 5): this is deliberately `pub(crate)` and
    /// must be called ONLY by the grabber thread (the event-tap callback), and
    /// only where the detector's view of the keyboard is known to be stale —
    /// currently the single point after the tap is re-enabled following a
    /// `kCGEventTapDisabledByTimeout`, where KeyUp/FlagsChanged events may have
    /// been missed while the tap was off. Making it fully `pub` would let
    /// unrelated paths (a future CLI command, a watchdog, a test harness) clear
    /// chord state mid-operation and silently drop a copy/cut/paste.
    pub(crate) fn reset(&mut self) {
        self.held.clear();
        self.tab_consumed = false;
        self.last_digit = None;
        self.keys_pressed_in_chord.clear();
    }
}

/// Tier gate rule (issue #27, review pt 1).
///
/// Tier 2 (the full chord evaluator) is engaged whenever AT LEAST ONE
/// qualifying modifier — Command or Option — is held, and falls back to tier 1
/// (the lightweight idle listener) ONLY when BOTH are absent. The gate is
/// derived from the complete modifier state, never from whichever single
/// modifier a given FlagsChanged toggled. That is what keeps a KeyDown arriving
/// between two separate modifier-release events from being evaluated against a
/// half-released chord and silently dropped in Idle.
pub(crate) fn tier_active(cmd_held: bool, opt_held: bool) -> bool {
    cmd_held || opt_held
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

    // ── Dynamic mode: Opt released before C ──────────────────────────────────

    #[test]
    fn dynamic_copy_when_opt_released_before_letter() {
        let mut detector = ChordDetector::new();

        detector.key_down(Key::MetaLeft);
        detector.key_down(Key::Alt);
        detector.key_down(Key::KeyC);

        // Opt releases slightly before C
        detector.key_up(Key::Alt);

        let action = detector.evaluate_release(&Key::KeyC, false);
        assert_eq!(action, Some(HotkeyAction::DynamicCopy));
    }

    #[test]
    fn dynamic_paste_when_opt_released_before_letter() {
        let mut detector = ChordDetector::new();

        detector.key_down(Key::MetaLeft);
        detector.key_down(Key::Alt);
        detector.key_down(Key::KeyV);

        detector.key_up(Key::Alt);

        let action = detector.evaluate_release(&Key::KeyV, false);
        assert_eq!(action, Some(HotkeyAction::DynamicPaste));
    }

    // ── Static mode: Opt released before C (regression for missing test) ─────

    #[test]
    fn static_copy_when_opt_released_before_letter() {
        let mut detector = ChordDetector::new();

        detector.key_down(Key::MetaLeft);
        detector.key_down(Key::Alt);
        // Press digit 3 to address slot
        detector.key_down(Key::Num3);
        // Now press C while chord still active
        detector.key_down(Key::KeyC);

        // Opt releases first — last_digit must survive for evaluate_release
        detector.key_up(Key::Alt);

        // C releases — should fire StaticCopy(3)
        let action = detector.evaluate_release(&Key::KeyC, true);
        assert_eq!(action, Some(HotkeyAction::StaticCopy(3)));
    }

    #[test]
    fn static_cut_when_opt_released_before_letter() {
        let mut detector = ChordDetector::new();

        detector.key_down(Key::MetaLeft);
        detector.key_down(Key::Alt);
        detector.key_down(Key::Num5);
        detector.key_down(Key::KeyX);

        detector.key_up(Key::Alt);

        let action = detector.evaluate_release(&Key::KeyX, true);
        assert_eq!(action, Some(HotkeyAction::StaticCut(5)));
    }

    #[test]
    fn static_paste_when_opt_released_before_letter() {
        let mut detector = ChordDetector::new();

        detector.key_down(Key::MetaLeft);
        detector.key_down(Key::Alt);
        detector.key_down(Key::Num7);
        detector.key_down(Key::KeyV);

        detector.key_up(Key::Alt);

        let action = detector.evaluate_release(&Key::KeyV, true);
        assert_eq!(action, Some(HotkeyAction::StaticPaste(7)));
    }

    // ── No false-positive when key not pressed in chord ───────────────────────

    #[test]
    fn no_action_when_key_not_in_chord() {
        let mut detector = ChordDetector::new();

        detector.key_down(Key::MetaLeft);
        detector.key_down(Key::Alt);
        // C was NOT pressed while chord active

        let action = detector.evaluate_release(&Key::KeyC, false);
        assert_eq!(action, None);
    }

    // ── Digit cleaned up after chord drains ──────────────────────────────────

    #[test]
    fn last_digit_cleared_after_chord_drains() {
        let mut detector = ChordDetector::new();

        detector.key_down(Key::MetaLeft);
        detector.key_down(Key::Alt);
        detector.key_down(Key::Num2);
        detector.key_down(Key::KeyC);

        detector.key_up(Key::Alt);
        // consume the pending key
        let _ = detector.evaluate_release(&Key::KeyC, true);

        // digit should now be cleared — a new chord must supply a fresh digit
        assert!(!detector.is_pending_chord_key(&Key::KeyC));
    }

    // ── Tier gate rule (issue #27, review pt 1) ──────────────────────────────

    #[test]
    fn tier_stays_active_until_both_modifiers_released() {
        // Cmd+Opt held → Active.
        assert!(tier_active(true, true));
        // Opt released first, Cmd still held → MUST remain Active so a KeyDown
        // in the gap is not dropped.
        assert!(tier_active(true, false));
        // Cmd released first, Opt still held → still Active.
        assert!(tier_active(false, true));
        // Both released → Idle.
        assert!(!tier_active(false, false));
    }

    // ── reset() ownership/behavior (issue #27, review pt 5) ──────────────────

    #[test]
    fn reset_clears_all_chord_state() {
        let mut detector = ChordDetector::new();

        detector.key_down(Key::MetaLeft);
        detector.key_down(Key::Alt);
        detector.key_down(Key::Num4);
        detector.key_down(Key::KeyC);
        assert!(detector.cmd() && detector.opt());
        assert!(detector.is_pending_chord_key(&Key::KeyC));

        detector.reset();

        assert!(!detector.cmd());
        assert!(!detector.opt());
        assert!(!detector.is_pending_chord_key(&Key::KeyC));
        // The stale digit must be gone: a fresh C release with no new digit is a no-op.
        assert_eq!(detector.evaluate_release(&Key::KeyC, true), None);
    }
}