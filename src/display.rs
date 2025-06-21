///
/// @package subtle-rs
///
/// @file Display functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use anyhow::{Result};
use x11rb::connection::Connection;
use crate::{Config, Subtle};

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    let (conn, screen_num) = x11rb::connect(Some(&*config.display))?;
    
    subtle.width = conn.setup().roots[screen_num].width_in_pixels;
    subtle.height = conn.setup().roots[screen_num].height_in_pixels;
    subtle.conn = Option::from(conn);

    println!("Display ({}) is {}x{}", config.display, subtle.width, subtle.height);

        //DisplayString(subtle->dpy), subtle->width, subtle->height);

    Ok(())
}

pub(crate) fn configure(_config: &Config, _subtle: &Subtle) -> Result<()> {
    Ok(())
}

pub(crate) fn finish(_subtle: &Subtle) -> Result<()> {
    Ok(())
}
