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

use crate::client::Client;
use crate::config::{Config, MixedConfigVal};
use crate::gravity::Gravity;
use crate::tag::Tag;
use crate::view::View;
use bitflags::bitflags;
use std::cell::{Cell, OnceCell};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use veccell::{VecCell, VecRef, VecRefMut};
use x11rb::connection::Connection;
use x11rb::NONE;
use x11rb::protocol::xproto::{ConnectionExt, Cursor, Gcontext, Grab, Window};
use x11rb::rust_connection::RustConnection;
use crate::ewmh::Atoms;
use crate::screen::Screen;
use crate::style::Style;
use crate::tagging::Tagging;

const HISTORY_SIZE: usize = 5;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct SubtleFlags: u32 {
        const DEBUG = 1 << 0; // Debug enabled
        const CHECK = 1 << 1; // Check config
        const URGENT = 1 << 2; // Urgent transients
        const RESIZE = 1 << 3; // Respect size hints

        const XINERAMA = 1 << 4; // Using Xinerama
        const XRANDR = 1 << 5; // Using Xrandr
        const XFT = 1 << 6; // Using XFT

        const EWMH = 1 << 7; // EWMH set
        const REPLACE = 1 << 8; // Replace previous wm
        const RESTART = 1 << 9; // Restart
        const RELOAD = 1 << 10; // Reload config
        const TRAY = 1 << 11; // Use tray
        const GRAVITY_TILING = 1 << 12; // Enable gravity tiling
        const FOCUS_CLICK = 1 << 13; // Click to focus
        const SKIP_POINTER_WARP = 1 << 14; // Skip pointer warp
        const SKIP_URGENT_WARP = 1 << 15; // Skip urgent warp
    }
}

#[derive(Default)]
pub(crate) struct SubtleStyles {
    pub(crate) all: Style,
    pub(crate) views: Style,
    pub(crate) title: Style,
    pub(crate) sublets: Style,
    pub(crate) separator: Style,
    pub(crate) clients: Style,
    pub(crate) panel_top: Style,
    pub(crate) panel_bottom: Style,
}

pub(crate) struct Subtle {
    pub(crate) flags: SubtleFlags,
    pub(crate) width: u16,
    pub(crate) height: u16,

    pub(crate) panel_height: u16,
    pub(crate) step_size: u16,
    pub(crate) snap_size: u16,
    pub(crate) default_gravity: isize,

    pub(crate) visible_tags: Cell<Tagging>,
    pub(crate) visible_views: Cell<Tagging>,
    pub(crate) client_tags: Cell<Tagging>,
    pub(crate) urgent_tags: Cell<Tagging>,

    pub(crate) exterminate: Arc<AtomicBool>,
    pub(crate) conn: OnceCell<RustConnection>,
    pub(crate) screen_num: usize,

    pub(crate) atoms: OnceCell<Atoms>,

    pub(crate) support_win: Window,
    pub(crate) tray_win: Window,
    pub(crate) focus_history: VecCell<Window>,

    pub(crate) invert_gc: Gcontext,
    pub(crate) draw_gc: Gcontext,

    pub(crate) arrow_cursor: Cursor,
    pub(crate) move_cursor: Cursor,
    pub(crate) resize_cursor: Cursor,

    pub(crate) styles: SubtleStyles,

    pub(crate) screens: Vec<Screen>,
    pub(crate) clients: VecCell<Client>,
    pub(crate) gravities: Vec<Gravity>,
    pub(crate) grabs: Vec<Grab>,
    pub(crate) tags: Vec<Tag>,
    pub(crate) views: Vec<View>,
}

impl Subtle {
    pub(crate) fn find_client(&self, win: Window) -> Option<VecRef<Client>> {
        self.clients.iter()
            .find(|client| client.win == win)
    }

    pub(crate) fn find_client_mut(&self, win: Window) -> Option<VecRefMut<Client>> {
        let maybe_idx = self.clients.iter()
            .position(|client| client.win == win);

        match maybe_idx {
            Some(idx) => self.clients.borrow_mut(idx),
            None => None,
        }
    }

    pub(crate) fn find_focus_client(&self) -> Option<VecRef<Client>> {
        if let Some(win) = self.focus_history.borrow(0) {
            if let Some(focus) = self.find_client(*win) {
                return self.find_client(*win)
            }
        }

        None
    }

    pub(crate) fn find_focus_win(&self) -> Window {
        if let Some(win) = self.focus_history.borrow(0) {
            return *win
        }

        NONE
    }

    pub(crate) fn find_screen_by_xy(&self, x: i16, y:i16) -> Option<(usize, &Screen)> {
        for (idx, screen) in self.screens.iter().enumerate() {
            if x >= screen.base.x && x < screen.base.x + screen.base.width as i16
                && y >= screen.base.y && y < screen.base.y + screen.base.height as i16
            {
                return Some((idx, &screen))
            }
        }
        
        None
    }

    pub(crate) fn find_screen_by_pointer(&self) -> Option<(usize, &Screen)> {
        // Check if there is only one screen
        if 1 == self.screens.len() {
            return self.screens.first().map(|screen| (0, screen))
        } else {
            let conn = self.conn.get().unwrap();

            let screen = &conn.setup().roots[self.screen_num];

            if let Ok(cookie) = conn.query_pointer(screen.root) {
                if let Ok(reply) = cookie.reply() {
                    return self.find_screen_by_xy(reply.root_x, reply.root_y)
                }
            }
        }

        None
    }
}

impl Default for Subtle {
    fn default() -> Self {
        Subtle {
            flags: SubtleFlags::empty(),
            width: 0,
            height: 0,

            panel_height: 0,
            step_size: 0,
            snap_size: 0,
            default_gravity: 0,
            
            visible_tags: Cell::new(Tagging::empty()),
            visible_views: Cell::new(Tagging::empty()),
            client_tags: Cell::new(Tagging::empty()),
            urgent_tags: Cell::new(Tagging::empty()),
            
            exterminate: Arc::new(AtomicBool::new(false)),
            conn: OnceCell::new(),
            screen_num: 0,
            
            atoms: OnceCell::new(),

            support_win: Window::default(),
            tray_win: Window::default(),
            focus_history: VecCell::with_capacity(HISTORY_SIZE),

            invert_gc: Gcontext::default(),
            draw_gc: Gcontext::default(),

            arrow_cursor: Cursor::default(),
            move_cursor: Cursor::default(),
            resize_cursor: Cursor::default(),

            styles: SubtleStyles::default(),

            screens: Vec::new(),
            clients: VecCell::new(),
            gravities: Vec::new(),
            grabs: Vec::new(),
            tags: Vec::new(),
            views: Vec::new(),
        }
    }
}


impl From<&Config> for Subtle {
    fn from(config: &Config) -> Self {
        let mut subtle = Self::default();

        // CLI options
        if config.replace {
            subtle.flags.insert(SubtleFlags::REPLACE);
        }

        // Config options
        if let Some(MixedConfigVal::I(step_size)) = config.subtle.get("increase_step") {
            subtle.step_size = *step_size as u16;
        }

        if let Some(MixedConfigVal::I(snap_size)) = config.subtle.get("border_snap") {
            subtle.snap_size = *snap_size as u16;
        }

        if let Some(MixedConfigVal::I(grav_id)) = config.subtle.get("default_gravity") {
            subtle.default_gravity = grav_id.clone() as isize;
        }

        macro_rules! apply_config_flag {
            ($config_key:expr, $subtle_flag:path) => {
                if let Some(MixedConfigVal::B(value)) = config.subtle.get($config_key) && *value {
                    subtle.flags.insert($subtle_flag);
                }
            };
        }

        apply_config_flag!("urgent_dialogs", SubtleFlags::URGENT);
        apply_config_flag!("honor_size_hints", SubtleFlags::RESIZE);
        apply_config_flag!("gravity_tiling", SubtleFlags::GRAVITY_TILING);
        apply_config_flag!("click_to_focus", SubtleFlags::FOCUS_CLICK);
        apply_config_flag!("skip_pointer_warp", SubtleFlags::SKIP_POINTER_WARP);
        apply_config_flag!("skip_urgent_warp", SubtleFlags::SKIP_URGENT_WARP);

        subtle
    }
}
