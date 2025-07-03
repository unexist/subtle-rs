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
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, PropMode, Rectangle, SetMode, Window};
use bitflags::bitflags;
use anyhow::{Result};
use easy_min_max::max;
use log::debug;
use x11rb::NONE;
use crate::ewmh::{Atoms, AtomsCookie};
use crate::subtle::Subtle;
use crate::tagging::Tagging;

const MIN_WIDTH: u16 = 1;
const MIN_HEIGHT: u16 = 1;

#[repr(u8)]
#[derive(Copy, Clone)]
pub(crate) enum WMState {
    WithdrawnState = 0,
    NormalState = 1,
}

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
    pub(crate) tags: Tagging,

    pub(crate) win: Window,
    pub(crate) leader: Window,

    pub(crate) name: String,
    pub(crate) instance: String,
    pub(crate) klass: String,
    pub(crate) role: String,

    pub(crate) min_ratio: f32,
    pub(crate) max_ratio: f32,

    pub(crate) min_width: i32,
    pub(crate) min_height: i32,
    pub(crate) max_width: i32,
    pub(crate) max_height: i32,
    pub(crate) inc_width: i32,
    pub(crate) inc_height: i32,
    pub(crate) base_width: i32,
    pub(crate) base_height: i32,

    pub(crate) screen_id: usize,
    pub(crate) gravity_id: usize,
    
    pub(crate) geom: Rectangle,

    pub(crate) gravities: Vec<usize>,
}

impl Client {
    pub(crate) fn new(subtle: &Subtle, win: Window) -> Result<Self> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        conn.grab_server()?;
        conn.change_save_set(SetMode::INSERT, win)?;
        
        let geom_reply = conn.get_geometry(win)?.reply()?;

        let wm_name = conn.get_property(false, win,
                                        atoms.WM_NAME, AtomEnum::STRING,
                                        0, 1024)?.reply()?.value;

        let wm_klass = conn.get_property(false, win, atoms.WM_CLASS,
                                         AtomEnum::STRING, 0, 1024)?.reply()?.value;

        let inst_klass = String::from_utf8(wm_klass)
            .expect("UTF-8 string should be valid UTF-8")
            .trim_matches('\0')
            .split('\0')
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        conn.ungrab_server()?;

        let mut client = Self {
            win,
            name: String::from_utf8(wm_name)?,
            instance: inst_klass[0].to_string(),
            klass: inst_klass[1].to_string(),
            geom: Rectangle {
                x: geom_reply.x,
                y: geom_reply.y,
                width: max!(MIN_WIDTH, geom_reply.width),
                height: max!(MIN_HEIGHT, geom_reply.height),
            },
            gravities: Vec::with_capacity(subtle.views.len()),
            ..Self::default()
        };

        // Update client
        let mut new_flags = Flags::empty();

        client.set_wm_state(subtle, WMState::WithdrawnState);
        client.retag(subtle, &mut new_flags);

        // Set leader window
        let leader = conn.get_property(false, client.win, AtomEnum::WINDOW,
                                       atoms.WM_CLIENT_LEADER, 0, 1)?.reply()?.value;

        if !leader.is_empty() && NONE != leader[0] as u32 {
            client.leader = leader[0] as Window;
        }

        debug!("New: {}", client);

        Ok(client)
    }

    pub(crate) fn set_wm_state(&self, subtle: &Subtle, state: WMState) {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        let data: [u8; 2] = [state as u8, NONE as u8];

        let _ = conn.change_property(PropMode::REPLACE,
                                     self.win, atoms.WM_STATE, atoms.WM_STATE, 8, 2, &data);
    }

    pub(crate) fn tag(&self, tag_idx: usize, new_flags: &mut Flags) {

    }

    pub(crate) fn retag(&self, subtle: &Subtle, new_flags: &mut Flags) {
        for (tag_idx, tag) in subtle.tags.iter().enumerate() {
            if tag.matches(self) {
                self.tag(tag_idx, new_flags);
            }
        }

        if self.flags.contains(Flags::MODE_STICK) && !new_flags.contains(Flags::MODE_STICK) {
            let mut visible: u8 = 0;

            for view in subtle.views.iter() {
                if view.tags.contains(self.tags) {
                    visible += 1;
                }
            }

            if 0 == visible {
                self.tag(0, new_flags);
            }
        }
    }

    pub(crate) fn map(&self, subtle: &Subtle) {
        let conn = subtle.conn.get().unwrap();

        let _ = conn.map_window(self.win);
    }

    pub(crate) fn unmap(&self, subtle: &Subtle) {
        let conn = subtle.conn.get().unwrap();

        let _ = conn.unmap_window(self.win);
    }
}

impl fmt::Display for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "name={}, instance={}, class={}, win={}, leader={}, geom=(x={}, y={}, width={}, height={})",
               self.name, self.instance, self.klass, self.win, self.leader,
               self.geom.x, self.geom.y, self.geom.width, self.geom.height)
    }
}