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
use itertools::Itertools;

#[host_fn("extism:host/user")]
extern "ExtismHost" {
    fn get_battery(battery_idx: String) -> String;
}

#[plugin_fn]
pub unsafe fn run<'a>() -> FnResult<String> {
    let values: String = unsafe { get_battery("0".into())? };

    info!("battery {}", values);

    let (charge_full, charge_now) = values.split(" ")
        .filter_map(|v| v.parse::<i32>().ok()).collect_tuple().or(Some((1, 0))).unwrap();

    Ok(format!("{}%", charge_now * 100 / charge_full))
}
