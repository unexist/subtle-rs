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
use crate::{Config, Subtle};

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    let (conn, _screen_num) = x11rb::connect(Some(&*config.display))?;
    
    subtle.conn = Option::from(conn);
    
    Ok(())
}

pub(crate) fn configure(_config: &Config, _subtle: &Subtle) -> Result<()> {
    Ok(())
}

pub(crate) fn finish(_subtle: &Subtle) -> Result<()> {
    Ok(())
}
