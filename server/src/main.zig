const std = @import("std");
const ws = @import("websocket");

pub fn main() !void {
    const allocator = std.heap.page_allocator;

    var app = App.init();
    defer app.deinit(allocator);

    var thread = try std.Thread.spawn(.{}, udpLoop, .{ allocator, &app });
    thread.detach();
    defer std.Thread.join(thread);

    var server = try ws.Server(Handler).init(allocator, .{
        .port = 3000,
        .address = "127.0.0.1",
        .handshake = .{
            .timeout = 3,
            .max_size = 1024,
            .max_headers = 0,
        },
    });

    try server.listen(&app);
}

const Handler = struct {
    app: *App,
    conn: *ws.Conn,

    pub fn init(h: *ws.Handshake, conn: *ws.Conn, app: *App) !Handler {
        _ = h;

        return .{
            .app = app,
            .conn = conn,
        };
    }

    pub fn clientMessage(self: *Handler, data: []const u8) !void {
        try self.conn.write(data);
    }
};

const App = struct {
    messages: std.ArrayList([]const u8),

    pub fn init() App {
        return .{
            .messages = .empty,
        };
    }

    pub fn deinit(a: *App, allocator: std.mem.Allocator) void {
        a.messages.deinit(allocator);
    }
};

fn udpLoop(allocator: std.mem.Allocator, app: *App) !void {
    const addr = try std.net.Address.parseIp("127.0.0.1", 34501);
    std.debug.print("info(udp): socket listening on {s}:{d}\n", .{ "127.0.0.1", addr.getPort() });

    const sock = try std.posix.socket(
        std.posix.AF.INET,
        std.posix.SOCK.DGRAM,
        std.posix.IPPROTO.UDP,
    );
    defer std.posix.close(sock);

    try std.posix.bind(sock, &addr.any, addr.getOsSockLen());

    var buf: [1024]u8 = undefined;
    var client_addr: std.posix.sockaddr = undefined;
    var client_len: std.posix.socklen_t = @sizeOf(std.posix.sockaddr);

    while (true) {
        const n = try std.posix.recvfrom(sock, &buf, 0, &client_addr, &client_len);

        const msg = buf[0..n];

        std.debug.print("info(udp) recieved message: {s}\n", .{msg});

        try app.messages.append(allocator, msg);

        _ = try std.posix.sendto(sock, msg, 0, &client_addr, client_len);
    }
}
