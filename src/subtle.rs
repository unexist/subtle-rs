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

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use bitflags::bitflags;
use x11rb::rust_connection::RustConnection;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct Flags: u32 {
        const DEBUG = 1 << 0;
        const CHECK = 1 << 1;
    }
}

#[derive(Default)]
pub(crate) struct Subtle {
    pub(crate) flags: Flags,
    pub(crate) running: Arc<AtomicBool>,
    pub(crate) conn: Option<RustConnection>,
}
