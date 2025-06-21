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
