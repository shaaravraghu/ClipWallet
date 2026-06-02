pub mod chord;
pub mod grabber;
pub mod injector;

pub use chord::{ChordDetector, HotkeyAction};
pub use injector::{simulate_copy, simulate_cut, simulate_paste, simulate_paste_delayed};
pub use grabber::{spawn_event_tap, GrabberHandle};