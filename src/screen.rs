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
use anyhow::Result;
use x11rb::protocol::xproto::Rectangle;
use crate::config::Config;
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

pub(crate) fn init(_config: &Config, _subtle: &mut Subtle) -> Result<()> {
    debug!("Init");

    Ok(())
}


pub(crate) fn render(subtle: &Subtle) {
    debug!("Render");
}
