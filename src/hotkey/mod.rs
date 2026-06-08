pub mod chord;
pub mod grabber;
pub mod injector;

pub use chord::{ChordDetector, HotkeyAction};
pub use injector::{simulate_copy, simulate_cut, simulate_paste, simulate_paste_delayed};
pub use grabber::{spawn_event_tap, GrabberHandle};

/// Sentinel written into the `EVENT_SOURCE_USER_DATA` field of every keyboard
/// event ClipWallet injects — copy, cut, *and* paste. The event tap reads this

pub(crate) const SYNTHETIC_TAG: i64 = 0x436C_6970_5741_4C54;

