///
/// @package subtle-rs
///
/// @file Gravity functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use bitflags::bitflags;
use easy_min_max::{min, max, clamp};
use anyhow::Result;
use log::debug;
use crate::Config;
use crate::rect::Rect;
use crate::subtle::Subtle;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct Flags: u32 {
        const HORZ = 1 << 0; // Gravity tile gravity horizontally
        const VERT = 1 << 1; // Gravity tile gravity vertically
    }
}

#[derive(Default)]
pub(crate) struct Gravity {
    pub(crate) flags: Flags,
    pub(crate) name: String,
    pub geom: Rect,
}

impl Gravity {
    pub fn new(name: String, x: u16, y: u16, width: u16, height: u16) -> Self {
        let grav = Gravity {
            flags: Flags::empty(),
            name,
            geom: Rect {
                x: clamp!(x, 0, 100),
                y: clamp!(y, 0, 100),
                width: clamp!(width, 1, 100),
                height: clamp!(height, 1, 100),
            }
        };
        
        debug!("New: {}", grav);
        
        grav
    }
}

impl fmt::Display for Gravity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "name={}, geom={}", self.name, self.geom)
    }
}

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    subtle.gravities = config.gravities.iter()
        .map(|grav| Gravity::new(String::from(grav.0), grav.1[0], grav.1[1], 
                                 grav.1[2], grav.1[3])).collect(); 
    
    Ok(())
}
