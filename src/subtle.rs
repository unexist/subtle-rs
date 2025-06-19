///
/// @package subtle-rs
///
/// @file Subtle functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use bitflags::bitflags;
use x11rb::rust_connection::RustConnection;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct Flags: u32 {
        const DEBUG = 1 << 0; // Debug enabled
        const CHECK = 1 << 1; // Check config
        const RUN = 1 << 2; // Run event loop
        const URGENT = 1 << 3; // Urgent transients
        const RESIZE = 1 << 4; // Respect size
        const XINERAMA = 1 << 5; // Using Xinerama
        const XRANDR = 1 << 6; // Using Xrandr
        const EWMH = 1 << 7; // EWMH set
        const REPLACE = 1 << 8; // Replace previous wm
        const RESTART = 1 << 9; // Restart
        const RELOAD = 1 << 10; // Reload config
        const TRAY = 1 << 11; // Use tray
        const TILING = 1 << 12; // Enable tiling
        const FOCUS_CLICK = 1 << 13; // Click to focus
        const SKIP_WARP = 1 << 14; // Skip pointer warp
        const SKIP_URGENT_WARP = 1 << 15; // Skip urgent warp
    }
}

#[derive(Default)]
pub(crate) struct Subtle {
    pub(crate) flags: Flags,
    pub(crate) running: Arc<AtomicBool>,
    pub(crate) conn: Option<RustConnection>,
}
