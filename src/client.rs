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

#[derive(Debug, Default)]
struct Client {
    win: Window,
    title: String,
}

impl Client {
    fn new(win: Window) -> Self {
        Client {
            win,
            ..Self::default()
        }
    }
}
