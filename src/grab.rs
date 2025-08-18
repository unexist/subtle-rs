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
use anyhow::{anyhow, Context, Result};
use log::debug;
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::NONE;
use x11rb::protocol::xproto::{ButtonIndex, ConnectionExt, EventMask, GrabMode, Keycode, ModMask, Window};
use crate::config::Config;
use crate::subtle::{Subtle, SubtleFlags};

bitflags! {
    #[derive(Default, Debug, Copy, Clone)]
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

    pub(crate) code: Keycode,
    pub(crate) modifiers: ModMask,

    pub(crate) app: Option<String>,
}

#[allow(non_upper_case_globals)]
const XK_Pointer_Button1: u32 = 0xfee9;

#[allow(non_upper_case_globals)]
const XK_a: u32 = 0x0061;

#[doc(hidden)]
pub(crate) fn parse_keys(keys: &str) -> Result<(u32, Keycode, ModMask, bool)> {
    let mut sym = 0u32;
    let mut code: Keycode = 0;
    let mut modifiers = ModMask::default();
    let mut is_mouse = false;

    for key in keys.split("-") {
        match key {
            "S" => modifiers |= ModMask::SHIFT,
            "C" => modifiers |= ModMask::CONTROL,
            "A" => modifiers |= ModMask::M1,
            "M" => modifiers |= ModMask::M3,
            "W" => modifiers |= ModMask::M4,
            "G" => modifiers |= ModMask::M5,
            _ => {
                if key.starts_with("B") {
                    let (_, btn) = key.split_at(1);

                    sym = XK_Pointer_Button1;
                    code = btn.parse::<Keycode>()?;
                    is_mouse = true;
                }
            }
        }
    }

    Ok((sym, code, modifiers, is_mouse))
}

#[doc(hidden)]
pub(crate) fn parse_name(name: &str) -> Result<GrabFlags> {
    Ok(match name {
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
        _ => return Err(anyhow!("Grab not found: {}", name))
    })
}

//const MASK_STATES: Vec<i32> = vec![0, ModMask::LOCK, numlockmask, numlockmask | ModMask::LOCK];


impl Grab {
    pub(crate) fn new(name: &str, keys: &str) -> Result<Self> {

        // Parse name and keys
        let key_flag = parse_name(name)?;
        let (sym, code, modifiers, is_mouse) = parse_keys(keys)?;

        let mut grab = Grab {
            flags: key_flag,
            modifiers: modifiers,
            ..Default::default()
        };

        if is_mouse {
            grab.flags.insert(GrabFlags::MOUSE);
        } else {
            grab.flags.insert(GrabFlags::KEY);
        }

        debug!("{}: {}", function_name!(), grab);

        Ok(grab)
    }

    pub(crate) fn set(subtle: Subtle, win: Window, grab_mask: GrabFlags) -> Result<()> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        let default_screen = &conn.setup().roots[subtle.screen_num];

        // Unbind click-to-focus grab
        if subtle.flags.intersects(SubtleFlags::FOCUS_CLICK) && default_screen.root != win {
            conn.ungrab_button(ButtonIndex::ANY, win, ModMask::ANY)?.check()?;
        }

        for grab in subtle.grabs.iter() {
            if grab.flags.intersects(grab_mask) {
                if grab.flags.intersects(GrabFlags::KEY) {
                    conn.grab_key(true, win, grab.modifiers, grab.code,
                                  GrabMode::ASYNC, GrabMode::ASYNC)?.check()?;
                } else if grab.flags.intersects(GrabFlags::MOUSE) {
                    conn.grab_button(false, win,
                                     EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE,
                                     GrabMode::ASYNC, GrabMode::ASYNC, NONE, NONE,
                                     ButtonIndex::try_from(grab.code)?,
                                     grab.modifiers)?.check()?;
                }

            }
        }

        debug!("{}", function_name!());

        Ok(())
    }

    pub(crate) fn unset(subtle: Subtle, win: Window) -> Result<()> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        let default_screen = &conn.setup().roots[subtle.screen_num];

        // pub const XCB_GRAB_ANY: xcb_grab_t = 0;
        conn.ungrab_key(Keycode::from(0), win, ModMask::ANY)?.check()?;
        conn.ungrab_button(ButtonIndex::ANY, win, ModMask::ANY)?.check()?;

        // Bind click-to-focus grab
        if subtle.flags.intersects(SubtleFlags::FOCUS_CLICK) && default_screen.root != win {
            conn.grab_button(false, win,
                             EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE,
                             GrabMode::ASYNC, GrabMode::ASYNC, NONE, NONE,
                             ButtonIndex::ANY, ModMask::ANY)?.check()?;
        }


        debug!("{}", function_name!());

        Ok(())
    }
}

impl fmt::Display for Grab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(code={}, state={:?}, app={:?})", self.code, self.modifiers, self.app)
    }
}

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    // Get modifier mask
    let reply = conn.get_modifier_mapping()?.reply()?;

    println!("keycodes: {:?}", reply.keycodes);

    // Parse grabs
    subtle.grabs = config.grabs.iter()
        .map(|grab| Grab::new(grab.0, grab.1))
        .filter_map(|res| res.ok()).collect();

    if 0 == subtle.gravities.len() {
        return Err(anyhow!("No grabs found"));
    }

    debug!("{}", function_name!());

    Ok(())
}
