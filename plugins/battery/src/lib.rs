#![no_main]

///
/// @package subtle-rs
///
/// @file Battery plugin functions
/// @copyright (c) 2026-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use extism_pdk::*;

#[host_fn("extism:host/user")]
extern "ExtismHost" {
    fn get_battery(battery_idx: String) -> String;
}

#[plugin_fn]
pub unsafe fn run<'a>() -> FnResult<String> {
    let output = unsafe { get_battery("0".into())? };

    Ok(output)
}
