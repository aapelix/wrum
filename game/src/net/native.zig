const std = @import("std");
const posix = std.posix;

const STATE = @import("state.zig").STATE;

pub var n = Socket{};

const Socket = struct {
    state: STATE = .CLOSED,
    buf: [1024]u8 = undefined,
    socket: std.posix.socket_t = undefined,
    addr: std.net.Address = undefined,

    pub fn init(self: *Socket) !void {
        const addr = try std.net.Address.parseIp("127.0.0.1", 34501);
        self.addr = addr;

        const sock = try posix.socket(posix.AF.INET, posix.SOCK.DGRAM | posix.SOCK.NONBLOCK, posix.IPPROTO.UDP);
        self.socket = sock;

        self.state = .OPEN;
    }

    pub fn deinit(self: *Socket) void {
        self.close();
        posix.close(self.socket);
    }

    pub fn send(self: *Socket, ptr: [*]const u8, len: usize) !void {
        if (self.state != .OPEN) {
            return;
        }

        const msg = ptr[0..len];
        _ = try posix.sendto(self.socket, msg, 0, &self.addr.any, self.addr.getOsSockLen());
    }

    pub fn recv(self: *Socket) !?[]const u8 {
        if (self.state != .OPEN) return null;

        var addr: std.net.Address = undefined;
        var addr_len: posix.socklen_t = @sizeOf(@TypeOf(addr.any));

        const recv_len = posix.recvfrom(
            self.socket,
            &self.buf,
            0,
            &addr.any,
            &addr_len,
        ) catch |err| switch (err) {
            error.WouldBlock => return null,
            else => return err,
        };

        if (recv_len == 0) return null;

        return self.buf[0..recv_len];
    }

    pub fn open(self: *Socket) void {
        self.state = .OPEN;
    }

    pub fn close(self: *Socket) void {
        self.state = .CLOSED;
    }
};
