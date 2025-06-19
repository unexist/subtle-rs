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

use bitflags::bitflags;
use crate::rect::Rect;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct Flags: u32 {
        const GRAVITY = 1 << 0; // Gravity property
        const GEOMETRY = 1 << 1; // Geometry property
        const POSITION = 1 << 2; // Position property
        const PROC = 1 << 3; // Tagging proc
    }
}

#[derive(Default)]
pub(crate) struct Tag {
    pub(crate) flags: Flags,
    pub(crate) name: String,
    
    pub(crate) screen_id: u32,
    pub(crate) gravity_id: u32,
    pub(crate) geom: Rect,
}
