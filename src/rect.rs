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
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub(crate) fn contains_point(&self, x: i16, y: i16) -> bool {
        x >= self.x
            &&  x as i32 <= self.x as i32 + self.width as i32
            && y >= self.y
            &&  y as i32 <= self.y as i32 + self.height as i32
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(x={}, y={}, width={}, height={})", 
               self.x, self.y, self.width, self.height)
    }
}

impl From<(i16, i16, u16, u16)> for Rect {
    fn from(rect: (i16, i16, u16, u16)) -> Self {
        Self {
            x: rect.0,
            y: rect.1,
            width: rect.2,
            height: rect.3,
        }
    }
}