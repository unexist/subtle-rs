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
    pub(crate) struct Tagging: u32 {}
}
