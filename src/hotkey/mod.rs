pub mod chord;
pub mod grabber;
pub mod injector;

pub use chord::{ChordDetector, HotkeyAction};
pub use injector::{simulate_copy, simulate_cut, simulate_paste, simulate_paste_delayed};

/// Sentinel written into the `EVENT_SOURCE_USER_DATA` field of every keyboard
/// event ClipWallet injects — copy, cut, *and* paste. The event tap reads this
/// field before anything else and passes any tagged event straight through, so
/// our own injections can never drive a tier transition or be re-interpreted as
/// a user chord. All three injection paths apply the same tag; a tag missing
/// from any one of them would let an injected key flip the tier mid-operation
/// (issue #27, review pt 6).
///
/// The value is the ASCII bytes "ClipWALT" — distinctive and overwhelmingly
/// unlikely to collide with another tool's user-data tag.
pub(crate) const SYNTHETIC_TAG: i64 = 0x436C_6970_5741_4C54;
