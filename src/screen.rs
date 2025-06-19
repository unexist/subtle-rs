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

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct Flags: u32 {
        const PANEL1 = 1 << 10; // Screen sanel1 enabled
        const PANEL2 = 1 << 11; // Screen sanel2 enabled
        const STIPPLE = 1 << 12; // Screen stipple enabled
        const VIRTUAL = 1 << 13; // Screen is virtual       
    }
}

#[derive(Default)]
pub(crate) struct Screen {
    pub(crate) flags: Flags,
}
