const std = @import("std");
const rl = @import("raylib");

pub const Stack = struct {
    textures: []rl.Texture,

    pub fn init(allocator: std.mem.Allocator, paths: []const [:0]const u8) !Stack {
        var textures = try allocator.alloc(rl.Texture, paths.len);

        for (paths, 0..) |path, i| {
            textures[i] = try rl.loadTexture(path);
        }

        return .{
            .textures = textures,
        };
    }

    pub fn fromFolder(allocator: std.mem.Allocator, path: []const u8, len: usize) !Stack {
        var paths = try allocator.alloc([:0]const u8, len);

        for (paths, 0..) |_, i| {
            const tmp = try std.fmt.allocPrint(allocator, "{s}/img_{d}.png", .{ path, i });
            paths[i] = try allocator.dupeZ(u8, tmp);
            allocator.free(tmp);
        }

        const stack = try Stack.init(allocator, paths);
        return stack;
    }

    pub fn deinit(self: *Stack, allocator: std.mem.Allocator) void {
        for (self.textures) |texture| {
            rl.unloadTexture(texture);
        }

        allocator.free(self.textures);
    }

    pub fn draw(self: *Stack, pos: rl.Vector2, r: f32) void {
        for (self.textures, 0..) |texture, i| {
            const width: f32 = @floatFromInt(texture.width);
            const height: f32 = @floatFromInt(texture.height);

            rl.drawTexturePro(texture, .{
                .x = 0,
                .y = 0,
                .width = width,
                .height = height,
            }, .{
                .x = pos.x,
                .y = pos.y - @as(f32, @floatFromInt(i)),
                .width = width,
                .height = height,
            }, .{
                .x = @floatFromInt(@divFloor(texture.width, 2)),
                .y = @floatFromInt(@divFloor(texture.height, 2)),
            }, r, .white);
        }
    }
};
