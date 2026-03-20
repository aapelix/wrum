const std = @import("std");
const STATE = @import("state.zig").STATE;

pub var n = WebSocket{};

const WebSocket = struct {
    state: STATE = .CLOSED,
    buf: [512]u8 = undefined,
    len: usize = 0,
    mutex: std.Thread.Mutex = std.Thread.Mutex{},

    pub fn deinit(self: *WebSocket) void {
        self.close();
    }

    pub fn on_message(self: *WebSocket, ptr: [*]const u8, len: usize) void {
        self.mutex.lock();
        defer self.mutex.unlock();

        const copy_len = @min(len, self.buf.len);
        @memcpy(self.buf[0..copy_len], ptr[0..copy_len]);
        self.len = copy_len;
    }

    pub fn send(self: *WebSocket, ptr: [*]const u8, len: usize) !void {
        if (self.state != .OPEN) {
            return;
        }

        send_ws(ptr, len);
    }

    pub fn open(self: *WebSocket) void {
        self.state = .OPEN;
    }

    pub fn close(self: *WebSocket) void {
        self.state = .CLOSED;
    }

    pub fn recv(self: *WebSocket) !?[]const u8 {
        self.mutex.lock();
        defer self.mutex.unlock();

        if (self.len == 0) {
            return null;
        }

        const msg = self.buf[0..self.len];
        self.len = 0;
        return msg;
    }
};

export fn ws_on_open() void {
    n.open();
}

export fn ws_on_close() void {
    n.close();
}

export fn ws_on_message(ptr: [*]const u8, len: usize) void {
    n.on_message(ptr, len);
}

pub extern fn init_ws() void;
extern fn send_ws(ptr: [*]const u8, len: usize) void;
