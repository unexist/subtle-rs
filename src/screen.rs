///
/// @package subtle-rs
///
/// @file View functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use bitflags::bitflags;
use log::debug;
use anyhow::{Context, Result};
use x11rb::connection::Connection;
use x11rb::CURRENT_TIME;
use x11rb::protocol::randr::ConnectionExt as randr_ext;
use x11rb::protocol::xinerama::ConnectionExt as xinerama_ext;
use x11rb::protocol::xproto::Rectangle;
use crate::config::Config;
use crate::subtle::Flags as SubtleFlags;
use crate::subtle::Subtle;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct Flags: u32 {
        const PANEL1 = 1 << 0; // Screen sanel1 enabled
        const PANEL2 = 1 << 1; // Screen sanel2 enabled
        const VIRTUAL = 1 << 3; // Screen is virtual       
    }
}

#[derive(Default)]
pub(crate) struct Screen {
    pub(crate) flags: Flags,

    pub(crate) geom: Rectangle,
    pub(crate) base: Rectangle,
}

impl Screen {
    pub(crate) fn new(subtle: &Subtle, x: i16, y: i16, width: u16, height: u16) -> Self {
        let screen = Self {
            flags: Flags::empty(),
            geom: Rectangle {
                x,
                y,
                width,
                height,
            },
            ..Self::default()
        };

        debug!("New: {}", screen);

        screen
    }
}

impl fmt::Display for Screen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "geom=(x={}, y={}, width={}, height={})",
               self.geom.x, self.geom.y, self.geom.width, self.geom.height)
    }
}

pub(crate) fn init(_config: &Config, subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    // Check both but prefer xrandr
    if subtle.flags.contains(SubtleFlags::XRANDR) {
        let screen = &conn.setup().roots[subtle.screen_num];
        let crtcs= conn.randr_get_screen_resources_current(screen.root)?.reply()?.crtcs;

        for crtc in crtcs {
            let screen_size = conn.randr_get_crtc_info(crtc, CURRENT_TIME)?.reply()?;

            let screen = Screen::new(subtle, screen_size.x, screen_size.y, screen_size.width, screen_size.height);

            subtle.screens.push(screen);
        }
    }

    if subtle.flags.contains(SubtleFlags::XINERAMA) && subtle.screens.is_empty() {
        if 0 != conn.xinerama_is_active()?.reply()?.state {
            let screens = conn.xinerama_query_screens()?.reply()?.screen_info;

            for screen_info in screens {
                let screen = Screen::new(subtle,
                                         screen_info.x_org, screen_info.y_org, screen_info.width, screen_info.height);

                subtle.screens.push(screen);
            }

        }
    }

    debug!("Init");

    Ok(())
}


pub(crate) fn render(subtle: &Subtle) {
    debug!("Render");
}
