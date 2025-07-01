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
use log::debug;
use crate::subtle::Subtle;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct Flags: u32 {
        const PANEL1 = 1 << 0; // Screen sanel1 enabled
        const PANEL2 = 1 << 1; // Screen sanel2 enabled
        const VIRTUAL = 1 << 3; // Screen is virtual       
    }
}

#[derive(Default)]
pub(crate) struct Screen {
    pub(crate) flags: Flags,
}

pub(crate) fn render(subtle: &Subtle) {
    debug!("Render");
}
