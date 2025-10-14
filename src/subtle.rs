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
use std::cell::{Cell, OnceCell, Ref, RefCell, RefMut};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use veccell::VecCell;
use x11rb::connection::Connection;
use x11rb::NONE;
use x11rb::protocol::xproto::{ConnectionExt, Cursor, Gcontext, Keycode, ModMask, Window};
use x11rb::rust_connection::RustConnection;
use crate::ewmh::Atoms;
use crate::font::Font;
use crate::grab::Grab;
use crate::screen::Screen;
use crate::style::Style;
use crate::tagging::Tagging;
use crate::tray::Tray;

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
        const CLICK_TO_FOCUS = 1 << 13; // Click to focus
        const SKIP_POINTER_WARP = 1 << 14; // Skip pointer warp
        const SKIP_URGENT_WARP = 1 << 15; // Skip urgent warp
    }
}

pub(crate) struct Subtle {
    pub(crate) flags: SubtleFlags,
    pub(crate) width: u16,
    pub(crate) height: u16,

    pub(crate) panel_height: u16,
    pub(crate) step_size: i16,
    pub(crate) snap_size: u16,
    pub(crate) default_gravity: isize,

    pub(crate) visible_tags: Cell<Tagging>,
    pub(crate) visible_views: Cell<Tagging>,
    pub(crate) client_tags: Cell<Tagging>,
    pub(crate) urgent_tags: Cell<Tagging>,

    pub(crate) shutdown: Arc<AtomicBool>,
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

    pub(crate) all_style: Style,
    pub(crate) views_style: Style,
    pub(crate) views_active_style: Style,
    pub(crate) views_occupied_style: Style,
    pub(crate) views_visible_style: Style,
    pub(crate) title_style: Style,
    pub(crate) urgent_style: Style,
    pub(crate) panels_style: Style,
    pub(crate) separator_style: Style,
    pub(crate) clients_style: Style,
    pub(crate) tray_style: Style,
    pub(crate) top_panel_style: Style,
    pub(crate) bottom_panel_style: Style,

    pub(crate) fonts: Vec<Font>,

    pub(crate) screens: Vec<Screen>,
    pub(crate) clients: RefCell<Vec<Client>>,
    pub(crate) trays: RefCell<Vec<Tray>>,
    pub(crate) gravities: Vec<Gravity>,
    pub(crate) grabs: Vec<Grab>,
    pub(crate) tags: Vec<Tag>,
    pub(crate) views: Vec<View>,
}

impl Subtle {
    pub(crate) fn find_client(&'_ self, win: Window) -> Option<Ref<'_, Client>> {
        Ref::filter_map(self.clients.borrow(), |clients| {
            clients.iter().find(|c| c.win == win)
        }).ok()
    }

    pub(crate) fn find_client_mut(&'_ self, win: Window) -> Option<RefMut<'_, Client>> {
        RefMut::filter_map(self.clients.borrow_mut(), |clients| {
            clients.iter_mut().find(|c| c.win == win)
        }).ok()
    }

    pub(crate) fn find_tray(&'_ self, win: Window) -> Option<Ref<'_, Tray>> {
        Ref::filter_map(self.trays.borrow(), |trays| {
            trays.iter().find(|t| t.win == win)
        }).ok()
    }

    pub(crate) fn find_tray_mut(&'_ self, win: Window) -> Option<RefMut<'_, Tray>> {
        RefMut::filter_map(self.trays.borrow_mut(), |trays| {
            trays.iter_mut().find(|c| c.win == win)
        }).ok()
    }

    pub(crate) fn find_focus_client(&'_ self) -> Option<Ref<'_, Client>> {
        if let Some(win) = self.focus_history.borrow(0) {
            return self.find_client(*win)
        }

        None
    }

    pub(crate) fn find_focus_client_mut(&'_ self) -> Option<RefMut<'_, Client>> {
        if let Some(win) = self.focus_history.borrow(0) {
            return self.find_client_mut(*win)
        }

        None
    }

    pub(crate) fn find_focus_win(&self) -> Window {
        if let Some(win) = self.focus_history.borrow(0)
            && NONE != *win
        {
            return *win
        }

        NONE
    }

    pub(crate) fn find_grab(&self, code: Keycode, modifiers: ModMask) -> Option<&Grab> {
        for grab in self.grabs.iter() {
            if grab.keycode == code && grab.modifiers == modifiers {
                return Some(grab);
            }
        }

        None
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

            let default_screen = &conn.setup().roots[self.screen_num];

            if let Ok(cookie) = conn.query_pointer(default_screen.root) {
                if let Ok(reply) = cookie.reply() {
                    return self.find_screen_by_xy(reply.root_x, reply.root_y);
                }
            }
        }

        None
    }

    pub(crate) fn find_screen_by_panel_win(&self, win: Window) -> Option<(usize, &Screen)> {
        for (screen_idx, screen) in self.screens.iter().enumerate() {
            if screen.top_panel_win == win || screen.bottom_panel_win == win {
                return Some((screen_idx, screen));
            }
        }

        None
    }

    pub(crate) fn add_client(&self, client: Client) {
        self.clients.borrow_mut().push(client);
    }

    pub(crate) fn remove_client_by_win(&self, win: Window) {
        self.clients.borrow_mut().retain(|c| c.win != win);
    }

    pub(crate) fn add_tray(&self, tray: Tray) {
        self.trays.borrow_mut().push(tray);
    }

    pub(crate) fn remove_tray_by_win(&self, win: Window) {
        self.trays.borrow_mut().retain(|t| t.win != win);
    }
}

impl Default for Subtle {
    fn default() -> Self {
        Subtle {
            flags: SubtleFlags::empty(),
            width: 0,
            height: 0,

            panel_height: 1,
            step_size: 0,
            snap_size: 0,
            default_gravity: 0,

            visible_tags: Cell::new(Tagging::empty()),
            visible_views: Cell::new(Tagging::empty()),
            client_tags: Cell::new(Tagging::empty()),
            urgent_tags: Cell::new(Tagging::empty()),

            shutdown: Arc::new(AtomicBool::new(false)),
            conn: OnceCell::new(),
            screen_num: 0,

            atoms: OnceCell::new(),

            support_win: Window::default(),
            tray_win: Window::default(),
            focus_history: VecCell::from(vec![NONE; HISTORY_SIZE]),

            invert_gc: Gcontext::default(),
            draw_gc: Gcontext::default(),

            arrow_cursor: Cursor::default(),
            move_cursor: Cursor::default(),
            resize_cursor: Cursor::default(),

            all_style: Style::default(),
            views_style: Style::default(),
            views_active_style: Style::default(),
            views_occupied_style: Style::default(),
            views_visible_style: Style::default(),
            title_style: Style::default(),
            urgent_style: Style::default(),
            panels_style: Style::default(),
            separator_style: Style::default(),
            clients_style: Style::default(),
            tray_style: Style::default(),
            top_panel_style: Style::default(),
            bottom_panel_style: Style::default(),

            fonts: Vec::new(),
            screens: Vec::new(),
            clients: RefCell::new(Vec::new()),
            trays: RefCell::new(Vec::new()),
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

        if config.debug {
            subtle.flags.insert(SubtleFlags::DEBUG);
        }

        // Config options
        if let Some(MixedConfigVal::I(step_size)) = config.subtle.get("increase_step") {
            subtle.step_size = *step_size as i16;
        }

        if let Some(MixedConfigVal::I(snap_size)) = config.subtle.get("border_snap") {
            subtle.snap_size = *snap_size as u16;
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
        apply_config_flag!("click_to_focus", SubtleFlags::CLICK_TO_FOCUS);
        apply_config_flag!("skip_pointer_warp", SubtleFlags::SKIP_POINTER_WARP);
        apply_config_flag!("skip_urgent_warp", SubtleFlags::SKIP_URGENT_WARP);

        subtle
    }
}
