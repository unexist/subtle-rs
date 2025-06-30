///
/// @package subtle-rs
///
/// @file Client functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, Rectangle, SetMode, Window};
use bitflags::bitflags;
use anyhow::{Result};
use log::debug;
use crate::ewmh::{Atoms, AtomsCookie};
use crate::subtle::Subtle;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct Flags: u32 {
        const DEAD = 1 << 0;  // Dead window
        const FOCUS = 1 << 1; // Send focus message
        const INPUT = 1 << 2; // Active/passive focus-model
        const CLOSE = 1 << 3; // Send close message
        const UNMAP = 1 << 4; // Ignore unmaps
        const ARRANGE = 1 << 5; // Re-arrange client

        const MODE_FULL = 1 << 6; // Fullscreen mode (also used in tags)
        const MODE_FLOAT = 1 << 7; // Float mode
        const MODE_STICK = 1 << 8; // Stick mode
        const MODE_STICK_SCREEN = 1 << 9; // Stick tagged screen mode
        const MODE_URGENT = 1 << 10; // Urgent mode
        const MODE_RESIZE = 1 << 11; // Resize mode
        const MODE_ZAPHOD = 1 << 12; // Zaphod mode
        const MODE_FIXED = 1 << 13; // Fixed size mode
        const MODE_CENTER = 1 << 14; // Center position mode
        const MODE_BORDERLESS = 1 << 15; // Borderless

        const TYPE_NORMAL = 1 << 16; // Normal type (also used in match)
        const TYPE_DESKTOP = 1 << 17; // Desktop type
        const TYPE_DOCK = 1 << 18; // Dock type
        const TYPE_TOOLBAR = 1 << 19; // Toolbar type
        const TYPE_SPLASH = 1 << 20; // Splash type
        const TYPE_DIALOG = 1 << 21; // Dialog type
    }
}

#[derive(Default, Debug)]
pub(crate) struct Client {
    pub(crate) flags: Flags,
    pub(crate) win: Window,
    pub(crate) title: String,
    pub(crate) instance: String,
    pub(crate) klass: String,
    
    pub(crate) geom: Rectangle,
}

impl Client {
    pub(crate) fn new(subtle: &Subtle, win: Window) -> Result<Self> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        conn.grab_server()?;
        conn.change_save_set(SetMode::INSERT, win)?;
        
        let geom_reply = conn.get_geometry(win)?.reply()?;

        let wm_name = conn.get_property(false, win,
                                        atoms.WM_NAME, atoms.UTF8_STRING,
                                        0, 1024)?.reply()?.value;
        conn.ungrab_server()?;

        let client = Client {
            win,
            title: String::from_utf8(wm_name)?,
            geom: Rectangle {
                x: geom_reply.x,
                y: geom_reply.y,
                width: geom_reply.width,
                height: geom_reply.height,
            },
            ..Self::default()
        };

        debug!("New: {}", client);

        Ok(client)
    }
}

impl fmt::Display for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "title={}", self.title)
    }
}