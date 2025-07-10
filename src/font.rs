///
/// @package subtle-rs
///
/// @file Font functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use crate::subtle::Subtle;

#[derive(Default, Debug)]
pub(crate) struct Font {
    pub(crate) y: u16,
    pub(crate) height: u16,
}

impl Font {
    pub(crate) fn new(subtle: &Subtle, name: &str) -> Self {
        Self {
            ..Default::default()
        }
    }
}

impl fmt::Display for Font {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(y={}, height={})",
               self.y, self.height)
    }
}
