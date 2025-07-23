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

use std::fmt;
use bitflags::bitflags;
use anyhow::{anyhow, Result};
use log::debug;
use stdext::function_name;
use x11rb::protocol::xproto::ModMask;
use crate::config::Config;
use crate::subtle::Subtle;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct GrabFlags: u32 {
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
    pub(crate) flags: GrabFlags,

    pub(crate) code: u16,
    pub(crate) state: u16,

    pub(crate) app: Option<String>,
}

#[allow(non_upper_case_globals)]
const XK_Pointer_Button1: u32 = 0xfee9;

#[allow(non_upper_case_globals)]
const XK_a: u32 = 0x0061;

#[doc(hidden)]
pub(crate) fn parse_keys(keys: &str) -> Result<(u32, u32, u32, bool)> {
    let mut sym = 0u32;
    let mut code = 0u32;
    let mut state = ModMask::default();
    let mut is_mouse = false;

    for key in keys.split("-") {
        match key {
            "S" => state |= ModMask::SHIFT,
            "C" => state |= ModMask::CONTROL,
            "A" => state |= ModMask::M1,
            "M" => state |= ModMask::M3,
            "W" => state |= ModMask::M4,
            "G" => state |= ModMask::M5,
            _ => {
                if key.starts_with("B") {
                    let (_, btn) = key.split_at(1);

                    sym = XK_Pointer_Button1;
                    code = XK_Pointer_Button1 + btn.parse::<u32>()?;
                    is_mouse = true;
                }
            }
        }
    }

    Ok((sym, code, u32::from(state), is_mouse))
}

#[doc(hidden)]
pub(crate) fn parse_name(name: &str) -> Result<GrabFlags> {
    let flag = match name {
        "view_focus" => GrabFlags::VIEW_FOCUS,
        "view_swap" => GrabFlags::VIEW_SWAP,
        "view_select" => GrabFlags::VIEW_SELECT,

        "screen_jump" => GrabFlags::SCREEN_JUMP,
        "subtle_reload" => GrabFlags::SUBTLE_RELOAD,
        "subtle_restart" => GrabFlags::SUBTLE_RESTART,
        "subtle_quit" => GrabFlags::SUBTLE_QUIT,

        "window_move" => GrabFlags::WINDOW_MOVE,
        "window_resize" => GrabFlags::WINDOW_RESIZE,
        "window_toggle" => GrabFlags::WINDOW_TOGGLE,
        "window_stack" => GrabFlags::WINDOW_STACK,
        "window_select" => GrabFlags::WINDOW_SELECT,
        "window_gravity" => GrabFlags::WINDOW_GRAVITY,
        "window_kill" => GrabFlags::WINDOW_KILL,
        _ => {
            GrabFlags::empty()
        }
    };

    if flag.is_empty() {
        Err(anyhow!("Grab not found: {}", name))
    } else {
        Ok(flag)
    }
}


impl Grab {
    pub(crate) fn new(name: &str, keys: &str) -> Result<Self> {

        // Parse name and keys
        let key_flag = parse_name(name)?;
        let (sym, code, state, is_mouse) = parse_keys(keys)?;

        let mut grab = Grab {
            flags: key_flag,
            state: state as u16,
            ..Self::default()
        };

        if is_mouse {
            grab.flags.insert(GrabFlags::MOUSE);
        } else {
            grab.flags.insert(GrabFlags::KEY);
        }

        debug!("{}: {}", function_name!(), grab);

        Ok(grab)
    }
}

impl fmt::Display for Grab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "code={}, state={}, app={:?}", self.code, self.state, self.app)
    }
}

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    subtle.grabs = config.grabs.iter()
        .map(|grab| Grab::new(grab.0, grab.1))
        .filter_map(|res| res.ok()).collect();

    // Check grabs
    if 0 == subtle.gravities.len() {
        return Err(anyhow!("No grabs found"));
    }

    debug!("{}", function_name!());

    Ok(())
}
