const builtin = @import("builtin");
const target = builtin.target.os.tag;

const client = if (target == .emscripten)
    @import("ws.zig")
else
    @import("native.zig");

pub fn init() !void {
    if (target == .emscripten)
        client.init_ws()
    else
        try client.n.init();
}

pub fn send(ptr: [*]const u8, len: usize) !void {
    try client.n.send(ptr, len);
}

pub fn recv() !?[]const u8 {
    return try client.n.recv();
}
