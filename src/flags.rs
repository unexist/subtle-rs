///
/// @package subtle-rs
///
/// @file Flags
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use bitflags::bitflags;

bitflags! {
    #[derive(Default)]
    pub(crate) struct SubtleFlags: u32 {
        const DEBUG = 1 << 0;
        const CHECK = 1 << 1;
    }
}