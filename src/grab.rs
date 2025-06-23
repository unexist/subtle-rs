///
/// @package subtle-rs
///
/// @file Grab functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use bitflags::bitflags;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct Flags: u32 {
        const KEY = 1 << 0; // Key grab
        const MOUSE = 1 << 1; // Mouse grab
        const SPAWN = 1 << 2; // Spawn an app
        const PROC = 1 << 3; // Grab with proc
        const CHAIN_START = 1 << 4; // Chain grab start
        const CHAIN_LINK = 1 << 5; // Chain grab link
        const CHAIN_END = 1 << 6; // Chain grab end
        const VIEW_FOCUS = 1 << 7; // Jump to view
        const VIEW_SWAP = 1 << 8; // Jump to view
        const VIEW_SELECT = 1 << 9; // Jump to view
        const SCREEN_JUMP = 1 << 10; // Jump to screen
        const SUBTLE_RELOAD = 1 << 11; // Reload subtle
        const SUBTLE_RESTART = 1 << 12; // Restart subtle
        const SUBTLE_QUIT = 1 << 13; // Quit subtle
        const WINDOW_MOVE = 1 << 14; // Resize window
        const WINDOW_RESIZE = 1 << 15; // Move window
        const WINDOW_TOGGLE = 1 << 16; // Toggle window
        const WINDOW_STACK = 1 << 17; // Stack window
        const WINDOW_SELECT = 1 << 18; // Select window
        const WINDOW_GRAVITY = 1 << 19; // Set gravity of window
        const WINDOW_KILL = 1 << 20; // Kill window

        /* Grab directions flags */
        const DIRECTION_UP = 1 << 0; // Direction up
        const DIRECTION_RIGHT = 1 << 1; // Direction right
        const DIRECTION_DOWN = 1 << 2; // Direction down
        const DIRECTION_LEFT = 1 << 3; // Direction left
    }
}

#[derive(Default)]
pub(crate) struct Grab {
    pub(crate) flags: Flags,

    pub(crate) code: u16,
    pub(crate) state: u16,

    pub(crate) app: Option<String>,
}
