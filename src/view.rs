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
        const MODE_ICON = 1 << 10; // View icon
        const MODE_ICON_ONLY = 1 << 11; // Icon only
        const MODE_DYNAMIC = 1 << 12; // Dynamic views
        const MODE_STICK = 1 << 13; // Stick view
    }
}

#[derive(Default)]
pub(crate) struct View {
    pub(crate) flags: Flags,
}
