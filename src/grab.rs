use std::collections::HashMap;
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
use x11rb::protocol::xproto::{ButtonIndex, ConnectionExt, EventMask, GrabMode, Keycode, Keysym, ModMask, Window};
use crate::config::Config;
use crate::subtle::{Subtle, SubtleFlags};

bitflags! {
    #[derive(Default, Debug, Copy, Clone)]
    pub(crate) struct GrabFlags: u32 {
        const IS_KEY = 1 << 0; // Key grab
        const IS_MOUSE = 1 << 1; // Mouse grab
        const SPAWN = 1 << 2; // Spawn an app
        const PROC = 1 << 3; // Grab with proc

        const CHAIN_START = 1 << 4; // Chain grab start
        const CHAIN_LINK = 1 << 5; // Chain grab link
        const CHAIN_END = 1 << 6; // Chain grab end

        const VIEW_JUMP = 1 << 7; // Jump to view
        const VIEW_SWITCH = 1 << 8; // Jump to view
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

#[derive(Default, Debug)]
pub(crate) struct Grab {
    pub(crate) flags: GrabFlags,

    pub(crate) code: Keycode,
    pub(crate) modifiers: ModMask,

    pub(crate) shell_cmd: Option<String>,
}

#[doc(hidden)]
pub(crate) fn parse_keys(keys: &str, keysyms_to_keycode: &HashMap<Keysym, Keycode>) -> Result<(Keycode, ModMask, bool)> {
    let mut code: Keycode = 0;
    let mut modifiers = ModMask::default();
    let mut is_mouse = false;

    println!("modifiers={:?}", modifiers);

    for key in keys.split("-") {
        match key {
            // Handle modifier keys
            "S" => modifiers |= ModMask::SHIFT,
            "C" => modifiers |= ModMask::CONTROL,
            "A" => modifiers |= ModMask::M1,
            "M" => modifiers |= ModMask::M3,
            "W" => modifiers |= ModMask::M4,
            "G" => modifiers |= ModMask::M5,
            _ => {
                // Handle mouse buttons
                if 2 == key.len() && key.starts_with("B") {
                    code = Keycode::from(ButtonIndex::try_from(
                        key.get(1..).unwrap()
                            .parse::<u8>().context("Parsing of mouse button failed")?)?);
                    is_mouse = true;
                // Handle other keys
                } else {
                    let record = x11_keysymdef::lookup_by_name(key).context("Keysym not found")?;

                    code = *keysyms_to_keycode.get(&record.keysym).context("Keycode not found")?;

                    println!("keys={}, record={:?}, keycode={}, modifieirs={:?}",
                             keys, record, code, modifiers);
                }
            }
        }
    }

    Ok((code, modifiers, is_mouse))
}

#[doc(hidden)]
pub(crate) fn parse_name(name: &str) -> Result<GrabFlags> {
    Ok(match name {
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
            // Handle grabs with index
            if name.starts_with("view_jump") {
                GrabFlags::VIEW_JUMP
            } else if name.starts_with("view_switch") {
                GrabFlags::VIEW_SWITCH
            } else if name.starts_with("screen_jump") {
                GrabFlags::SCREEN_JUMP
            } else {
                return Err(anyhow!("Grab not found: {}", name))
            }
        }
    })
}

impl Grab {
    pub(crate) fn new(name: &str, keys: &str, keysyms_to_keycode: &HashMap<Keysym, Keycode>) -> Result<Self> {

        // Parse name and keys
        let flags = parse_name(name)?;
        let (code, modifiers, is_mouse) = parse_keys(keys, keysyms_to_keycode)?;

        let grab = Grab {
            flags: flags | if is_mouse { GrabFlags::IS_MOUSE } else { GrabFlags::IS_KEY },
            code,
            modifiers,
            ..Default::default()
        };

        println!("{}: name={}, grab={}", function_name!(), name, grab);

        Ok(grab)
    }
}

impl fmt::Display for Grab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(flags={:?}, code={}, state={:?}, app={:?})",
               self.flags, self.code, self.modifiers, self.shell_cmd)
    }
}

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    // Get keyboard mapping
    let mapping = conn.get_keyboard_mapping(conn.setup().min_keycode,
        conn.setup().max_keycode - conn.setup().min_keycode + 1)?.reply()?;

    // Build reverse map of keysyms to keycode
    let mut keysyms_to_keycode = HashMap::new();

    for (idx, chunk) in mapping.keysyms
        .chunks(mapping.keysyms_per_keycode as usize)
        .enumerate()
    {
        let keycode = conn.setup().min_keycode + idx as u8;

        // Just copy the first sym without modifiers
        if let Some(&keysym) = chunk.first() && 0 != keycode {
            keysyms_to_keycode.insert(keysym, keycode);
        }
    }

    // Parse grabs
    subtle.grabs = config.grabs.iter()
        .map(|(grab_name, grab_keys)| {
            Grab::new(grab_name, grab_keys, &keysyms_to_keycode)
        })
        .filter_map(|res| res.ok())
        .collect();

    if 0 == subtle.gravities.len() {
        return Err(anyhow!("No grabs found"));
    }

    debug!("{}", function_name!());

    Ok(())
}

pub(crate) fn set(subtle: &Subtle, win: Window, grab_mask: GrabFlags) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    let default_screen = &conn.setup().roots[subtle.screen_num];

    // Unbind click-to-focus grab
    if subtle.flags.intersects(SubtleFlags::CLICK_TO_FOCUS) && default_screen.root != win {
        conn.ungrab_button(ButtonIndex::ANY, win, ModMask::ANY)?.check()?;
    }

    let mod_states: [ModMask; 4] = [ModMask::from(0u16),
        ModMask::LOCK, // Scrolllock
        ModMask::M2, // Numlock
        ModMask::M2 | ModMask::LOCK];

    // Bind grabs
    for grab in subtle.grabs.iter() {
        if grab.flags.intersects(grab_mask) {

            // FIXME: Ugly key/state grabbing
            for mod_state in mod_states.iter() {
                if grab.flags.intersects(GrabFlags::IS_KEY) {
                    conn.grab_key(true, default_screen.root,
                                  //ModMask::ANY, grab.code,
                                  grab.modifiers | *mod_state, grab.code,
                                  //grab.modifiers | *mod_state, grab.code,
                                  GrabMode::ASYNC, GrabMode::ASYNC)?.check()?;
                } else if grab.flags.intersects(GrabFlags::IS_MOUSE) {
                    conn.grab_button(false, win,
                                     EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE,
                                     GrabMode::ASYNC, GrabMode::ASYNC, NONE, NONE,
                                     ButtonIndex::from(grab.code),
                                     grab.modifiers | *mod_state)?.check()?;
                }
            }
        }
    }

    println!("{}: win={}, mask={:?}", function_name!(), win, grab_mask);

    Ok(())
}

pub(crate) fn unset(subtle: &Subtle, win: Window) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    let default_screen = &conn.setup().roots[subtle.screen_num];

    // pub const XCB_GRAB_ANY: xcb_grab_t = 0;
    conn.ungrab_key(Keycode::from(0), win, ModMask::ANY)?.check()?;
    conn.ungrab_button(ButtonIndex::ANY, win, ModMask::ANY)?.check()?;

    // Bind click-to-focus grab
    if subtle.flags.intersects(SubtleFlags::CLICK_TO_FOCUS) && default_screen.root != win {
        conn.grab_button(false, win,
                         EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE,
                         GrabMode::ASYNC, GrabMode::ASYNC, NONE, NONE,
                         ButtonIndex::ANY, ModMask::ANY)?.check()?;
    }


    debug!("{}", function_name!());

    Ok(())
}

