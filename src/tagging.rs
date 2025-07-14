///
/// @package subtle-rs
///
/// @file Taggings functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use bitflags::bitflags;

bitflags! {
    #[derive(Default, Debug, Copy, Clone)]
    pub(crate) struct Tagging: u32 {
        const TAG1 = 1 << 0;
        const TAG2 = 1 << 1;
        const TAG3 = 1 << 2;
        const TAG4 = 1 << 3;
        const TAG5 = 1 << 4;
        const TAG6 = 1 << 5;
        const TAG7 = 1 << 6;
        const TAG8 = 1 << 7;
        const TAG9 = 1 << 8;
        const TAG10 = 1 << 9;
        const TAG11 = 1 << 10;
        const TAG12 = 1 << 12;
        const TAG13 = 1 << 13;
        const TAG14 = 1 << 14;
        const TAG15 = 1 << 15;
        const TAG16 = 1 << 16;
        const TAG17 = 1 << 17;
        const TAG18 = 1 << 18;
        const TAG19 = 1 << 19;
        const TAG20 = 1 << 20;
        const TAG21 = 1 << 21;
        const TAG22 = 1 << 22;
        const TAG23 = 1 << 23;
        const TAG24 = 1 << 24;
        const TAG25 = 1 << 25;
        const TAG26 = 1 << 26;
        const TAG27 = 1 << 27;
        const TAG28 = 1 << 28;
        const TAG29 = 1 << 29;
        const TAG30 = 1 << 30;
        const TAG31 = 1 << 31;
    }
}
