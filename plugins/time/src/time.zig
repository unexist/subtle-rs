//
// @package subtle-rs
//
// @file Time plugin functions
// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
// @version $Id$
//
// This program can be distributed under the terms of the GNU GPLv3.
// See the file LICENSE for details.
//

const std = @import("std");
const extism_pdk = @import("extism-pdk");

const Plugin = extism_pdk.Plugin;
const alloc = std.heap.wasm_allocator;

pub extern "extism:host/user" fn get_formatted_time(u64) u64;

export fn run() i32 {
    const plugin = Plugin.init(alloc);

    const format = "[hour]:[minute]:[second]";
    const mem = plugin.allocateBytes(format);
    defer mem.free();

    const ptr = get_formatted_time(mem.offset);
    const rmem = plugin.findMemory(ptr);

    const buffer = plugin.allocator.alloc(u8, @intCast(rmem.length)) catch unreachable;

    rmem.load(buffer);

    plugin.outputMemory(rmem);

    return 0;
}
