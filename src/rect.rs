///
/// @package subtle-rs
///
/// @file Rect functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;

#[derive(Default)]
pub(crate) struct Rect {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(x={}, y={}, width={}, height={})", 
               self.x, self.y, self.width, self.height)
    }
}
