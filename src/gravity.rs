use std::fmt;
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

use std::fmt::Display;
use bitflags::bitflags;
use easy_min_max::{min, max, clamp};
use anyhow::Result;
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
    pub(crate) geom: Rect,
}

impl Gravity {
    fn new(name: String, x: u32, y: u32, width: u32, height: u32) -> Self {
        Gravity {
            flags: Flags::empty(),
            name,
            geom: Rect {
                x: clamp!(x, 0, 100),
                y: clamp!(y, 0, 100),
                width: clamp!(width, 1, 100),
                height: clamp!(height, 1, 100),
            }
        }
    }
}

impl Display for Gravity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "New: name={}, geom={}", self.name, self.geom)
    }
}

pub(crate) fn init(config: &Config, _subtle: &Subtle) -> Result<()> {
    for g in config.gravities.iter() {
        println!("{:?}", g);    
    }
    
    Ok(())
}
