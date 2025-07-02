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
use crate::config::Config;
use crate::gravity::Gravity;
use crate::tag::Tag;
use crate::view::View;
use bitflags::bitflags;
use std::cell::OnceCell;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use x11rb::protocol::xproto::{Grab, Window};
use x11rb::rust_connection::RustConnection;
use crate::ewmh::Atoms;
use crate::screen::Screen;
use crate::tagging::Tagging;

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

pub(crate) struct Subtle {
    pub(crate) flags: Flags,
    pub(crate) width: u16,
    pub(crate) height: u16,
    
    pub(crate) visible_tags: Tagging,
    pub(crate) visible_views: Tagging,
    pub(crate) client_tags: Tagging,
    pub(crate) urgent_tags: Tagging,
    
    pub(crate) exterminate: Arc<AtomicBool>,
    pub(crate) conn: OnceCell<RustConnection>,
    pub(crate) screen_num: usize,

    pub(crate) atoms: OnceCell<Atoms>,
    
    pub(crate) support_win: Window,
    pub(crate) tray_win: Window,

    pub(crate) screens: Vec<Screen>,
    pub(crate) clients: Vec<Client>,
    pub(crate) gravities: Vec<Gravity>,
    pub(crate) grabs: Vec<Grab>,
    pub(crate) tags: Vec<Tag>,
    pub(crate) views: Vec<View>,
}

impl Subtle {
    pub(crate) fn find_client(&self, win: Window) -> Option<&Client> {
        self.clients.iter()
            .find(|c| c.win == win)
    }
    
    pub(crate) fn find_screen(&self, x: i16, y:i16) -> Option<(usize, &Screen)> {
        for (idx, screen) in self.screens.iter().enumerate() {
            if x >= screen.base.x && x < screen.base.x + screen.base.width as i16
                && y >= screen.base.y && y < screen.base.y + screen.base.height as i16
            {
                return Some((idx, &screen))
            }
        }
        
        None
    }
}

impl Default for Subtle {
    fn default() -> Self {
        Subtle {
            flags: Flags::empty(),
            width: 0,
            height: 0,

            visible_tags: Tagging::empty(),
            visible_views: Tagging::empty(),
            client_tags: Tagging::empty(),
            urgent_tags: Tagging::empty(),
            
            exterminate: Arc::new(AtomicBool::new(false)),
            conn: OnceCell::new(),
            screen_num: 0,
            
            atoms: OnceCell::new(),

            support_win: Window::default(),
            tray_win: Window::default(),

            screens: Vec::new(),
            clients: Vec::new(),
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

        if config.replace {
            subtle.flags.insert(Flags::REPLACE);
        }
        
        subtle
    }
}
