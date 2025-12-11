const std = @import("std");
const extism_pdk = @import("extism-pdk");
//const ctime = @cImport(@cInclude("time.h"));

//const warn = @import("std").debug.warn;

const Plugin = extism_pdk.Plugin;
const alloc = std.heap.wasm_allocator;

export fn run() i32 {
    const plugin = Plugin.init(alloc);

    //const curtime = ctime.time(null);
    //const ltime = ctime.localtime(&curtime);

    //var buf: [40]u8 = undefined;

    //const format = "%a %b %e %H:%M:%S %Z %Y";
    //_ = ctime.strftime(&buf, buf.len, format, ltime);

    //warn("{}\n", buf);

    plugin.output("Ten past one");

    return 0;
}
