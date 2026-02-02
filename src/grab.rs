///
/// @package subtle-rs
///
/// @file Grab functions
/// @copyright (c) 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use std::collections::HashMap;
use bitflags::bitflags;
use anyhow::{anyhow, Context, Result};
use log::debug;
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::NONE;
use x11rb::protocol::xproto::{ButtonIndex, ConnectionExt, EventMask, GrabMode, Keycode, Keysym, ModMask, Window};
use crate::client;
use crate::client::ClientFlags;
use crate::config::{Config, MixedConfigVal};
use crate::subtle::{Subtle, SubtleFlags};

bitflags! {
    /// Config and state-flags for [`Grab`]
    #[derive(Default, Debug, Copy, Clone, PartialEq)]
    pub(crate) struct GrabFlags: u32 {
        /// Key grab
        const IS_KEY = 1 << 0;
        /// Mouse grab
        const IS_MOUSE = 1 << 1;
        /// Run a command
        const COMMAND = 1 << 2;
        /// Jump to view
        const VIEW_JUMP = 1 << 3;
        /// Jump to view
        const VIEW_SWITCH = 1 << 4;
        /// Jump to view
        const VIEW_SELECT = 1 << 5;
        /// Jump to screen
        const SCREEN_JUMP = 1 << 6;
        /// Reload subtle
        const SUBTLE_RELOAD = 1 << 7;
        /// Restart subtle
        const SUBTLE_RESTART = 1 << 8;
        /// Quit subtle
        const SUBTLE_QUIT = 1 << 9;
        /// Move window
        const WINDOW_MOVE = 1 << 10;
        /// Resize window
        const WINDOW_RESIZE = 1 << 11;
        /// Toggle window mode
        const WINDOW_MODE = 1 << 12;
        /// Restack window
        const WINDOW_RESTACK = 1 << 13;
        /// Select window
        const WINDOW_SELECT = 1 << 14;
        /// Set gravity of window
        const WINDOW_GRAVITY = 1 << 15;
        /// Kill window
        const WINDOW_KILL = 1 << 16;
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub(crate) enum DirectionOrder {
    Mouse = 0,
    Up = 1,
    Right = 2,
    Down = 3,
    Left = 4,
}

#[derive(Default, Debug)]
pub(crate) enum GrabAction {
    #[default]
    None,
    Index(u32),
    List(Vec<usize>),
    Command(String),
}

#[derive(Default, Debug)]
pub(crate) struct Grab {
    /// Config and state-flags
    pub(crate) flags: GrabFlags,
    /// Keycode of the grab
    pub(crate) keycode: Keycode,
    /// Modifier mask
    pub(crate) modifiers: ModMask,
    /// Action of this grab
    pub(crate) action: GrabAction,
}

/// Parse keys of grabs
///
/// # Arguments
///
/// * `keys` - Keys to parse
/// * `keysyms_to_keycode` - Mapping table for keysyms to keycode
///
/// # Returns
///
/// A [`Result`] with either ([`Keycode`], [`ModMask`], [`bool`]) on success or otherwise [`anyhow::Error`]
pub(crate) fn parse_keys(keys: &str, keysyms_to_keycode: &HashMap<Keysym, Keycode>) -> Result<(Keycode, ModMask, bool)> {
    let mut keycode: Keycode = 0;
    let mut modifiers = ModMask::default();
    let mut is_mouse = false;

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
                    keycode = Keycode::from(ButtonIndex::try_from(
                        key.get(1..).unwrap()
                            .parse::<u8>().context("Parsing of mouse button failed")?)?);
                    is_mouse = true;
                // Handle other keys
                } else {
                    let record = x11_keysymdef::lookup_by_name(key)
                        .context(format!("Key name not found: {}", key))?;

                    keycode = *keysyms_to_keycode.get(&record.keysym).context("Keysym not found")?;
                }
            }
        }
    }

    Ok((keycode, modifiers, is_mouse))
}

/// Parse names of grabs
///
/// # Arguments
///
/// * `str` - Name to parse
///
/// # Returns
///
/// A [`Result`] with either ([`GrabFlags`], [`GrabAction`]) on success or otherwise [`anyhow::Error`]
pub(crate) fn parse_name(name: &str) -> Result<(GrabFlags, GrabAction)> {
    Ok(match name {
        "subtle_reload" => (GrabFlags::SUBTLE_RELOAD, GrabAction::None),
        "subtle_restart" => (GrabFlags::SUBTLE_RESTART, GrabAction::None),
        "subtle_quit" => (GrabFlags::SUBTLE_QUIT, GrabAction::None),

        "window_toggle" => (GrabFlags::WINDOW_MODE, GrabAction::None),
        "window_stack" => (GrabFlags::WINDOW_RESTACK, GrabAction::None),
        "window_select" => (GrabFlags::WINDOW_SELECT, GrabAction::None),
        "window_gravity" => (GrabFlags::WINDOW_GRAVITY, GrabAction::None),
        "window_kill" => (GrabFlags::WINDOW_KILL, GrabAction::None),

        // Window modes
        "window_float" => (GrabFlags::WINDOW_MODE, GrabAction::Index(ClientFlags::MODE_FLOAT.bits())),
        "window_full" => (GrabFlags::WINDOW_MODE, GrabAction::Index(ClientFlags::MODE_FULL.bits())),
        "window_stick" => (GrabFlags::WINDOW_MODE, GrabAction::Index(ClientFlags::MODE_STICK.bits())),
        "window_zaphod" => (GrabFlags::WINDOW_MODE, GrabAction::Index(ClientFlags::MODE_ZAPHOD.bits())),

        // Window restack
        "window_raise" => (GrabFlags::WINDOW_RESTACK,
                           GrabAction::Index(client::RestackOrder::Up as u32)),
        "window_lower" => (GrabFlags::WINDOW_RESTACK,
                           GrabAction::Index(client::RestackOrder::Down as u32)),

        // Window select
        "window_left" => (GrabFlags::WINDOW_SELECT, GrabAction::Index(DirectionOrder::Left as u32)),
        "window_down" => (GrabFlags::WINDOW_SELECT, GrabAction::Index(DirectionOrder::Down as u32)),
        "window_right" => (GrabFlags::WINDOW_SELECT, GrabAction::Index(DirectionOrder::Right as u32)),
        "window_up" => (GrabFlags::WINDOW_SELECT, GrabAction::Index(DirectionOrder::Up as u32)),

        // Window dragging
        "window_move" => (GrabFlags::WINDOW_MOVE, GrabAction::None),
        "window_resize" => (GrabFlags::WINDOW_RESIZE, GrabAction::None),

        _ => {
            // Handle grabs with index
            if name.starts_with("view_jump") {
                (GrabFlags::VIEW_JUMP, GrabAction::Index(name[9..].parse()?))
            } else if name.starts_with("view_switch") {
                (GrabFlags::VIEW_SWITCH, GrabAction::Index(name[11..].parse()?))
            } else if name.starts_with("screen_jump") {
                (GrabFlags::SCREEN_JUMP, GrabAction::Index(name[11..].parse()?))
            } else {
                (GrabFlags::COMMAND, GrabAction::Command(name.to_string()))
            }
        }
    })
}

impl Grab {
    /// Create a new instance
    ///
    /// # Arguments
    ///
    /// * `name` - Name of this Grav
    /// * `keys` - Keys as String (A-F5)
    /// * `keysyms_to_keycode` - Lookup table to map keysyms to keycodes
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`Grab`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn new(name: &str, keys: &str, keysyms_to_keycode: &HashMap<Keysym, Keycode>) -> Result<Self> {

        // Parse name and keys
        let (flags, action) = parse_name(name)?;
        let (keycode, modifiers, is_mouse) = parse_keys(keys, keysyms_to_keycode)?;

        let grab = Grab {
            flags: flags | if is_mouse { GrabFlags::IS_MOUSE } else { GrabFlags::IS_KEY },
            keycode,
            modifiers,
            action,
            ..Default::default()
        };

        debug!("{}: name={}, grab={}", function_name!(), name, grab);

        Ok(grab)
    }
}

impl fmt::Display for Grab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(flags={:?}, code={}, state={:?}, app={:?})",
               self.flags, self.keycode, self.modifiers, self.action)
    }
}

/// Check config and init all gravity related options
///
/// # Arguments
///
/// * `config` - Config values read either from args or config file
/// * `subtle` - Global state object
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
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
    for (grab_name, value) in config.grabs.iter() {
        match value {
            MixedConfigVal::S(grab_keys) => {
                if let Ok(grab) = Grab::new(grab_name, grab_keys, &keysyms_to_keycode) {
                    subtle.grabs.push(grab);
                }
            }
            MixedConfigVal::M(items) => {
                for (grab_keys, gravities) in items.iter() {
                    if let Ok(mut grab) = Grab::new("window_gravity", grab_keys, &keysyms_to_keycode) {
                        let mut gravity_ids = Vec::with_capacity(gravities.len());

                        for grav_name in gravities {
                            if let Some(grav_id) = subtle.gravities.iter()
                                .position(|grav| grav.name.eq(grav_name))
                            {
                                gravity_ids.push(grav_id);
                            }
                        }

                        grab.action = GrabAction::List(gravity_ids);

                        subtle.grabs.push(grab);
                    }
                }
            }
            _ => {}
        }
    }

    if 0 == subtle.gravities.len() {
        return Err(anyhow!("No grabs found"));
    }

    debug!("{}", function_name!());

    Ok(())
}

/// Set active grabs on given window
///
/// # Arguments
///
/// * `subtle` - Global state object
/// * `win` - Window to use
/// * `grab_mask` - Grab mask
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
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
                                  grab.modifiers | *mod_state, grab.keycode,
                                  GrabMode::ASYNC, GrabMode::ASYNC)?.check()?;
                } else if grab.flags.intersects(GrabFlags::IS_MOUSE) {
                    conn.grab_button(false, win,
                                     EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE,
                                     GrabMode::ASYNC, GrabMode::ASYNC, NONE, NONE,
                                     ButtonIndex::from(grab.keycode),
                                     grab.modifiers | *mod_state)?.check()?;
                }
            }
        }
    }

    debug!("{}: win={}, mask={:?}", function_name!(), win, grab_mask);

    Ok(())
}

/// Unset active grabs on given window
///
/// # Arguments
///
/// * `subtle` - Global state object
/// * `win` - Window to use
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
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

