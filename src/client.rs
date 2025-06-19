///
/// @package subtle-rs
///
/// @file Client functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use x11rb::protocol::xproto::Window;
use bitflags::bitflags;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct Flags: u32 {
        const DEAD = 1 << 0;
        const FOCUS = 1 << 1;
    }
}

#[derive(Default, Debug)]
pub(crate) struct Client {
    pub flags: Flags,
    pub win: Window,
    pub title: String,
}

impl Client {
    pub(crate) fn new(win: Window) -> Self {
        Client {
            win,
            ..Self::default()
        }
    }
}
