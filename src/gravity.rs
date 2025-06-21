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

use bitflags::bitflags;
use easy_min_max::{min, max, clamp};
use crate::rect::Rect;

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
    pub(crate) quark: u32,
    pub(crate) geom: Rect,
}

impl Gravity {
    fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Gravity {
            flags: Flags::empty(),
            quark: 0,
            geom: Rect {
                x: clamp!(x, 0, 100),
                y: clamp!(y, 0, 100),
                width: clamp!(width, 1, 100),
                height: clamp!(height, 1, 100),
            }
        }
    }
}