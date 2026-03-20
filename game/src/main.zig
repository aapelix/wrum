const std = @import("std");
const rl = @import("raylib");

const net = @import("net/comb.zig");
const stack = @import("stack.zig").Stack;

const allocator = std.heap.page_allocator;

const screenWidth = 1280;
const screenHeight = 720;
const scale = 4;

const canvasWidth = screenWidth / scale;
const canvasHeight = screenHeight / scale;

pub fn main() anyerror!void {
    rl.initWindow(screenWidth, screenHeight, "wrum");
    defer rl.closeWindow();

    rl.setTargetFPS(60);

    try net.init();

    var fps_buf: [32]u8 = undefined;

    const canvas = try rl.loadRenderTexture(canvasWidth, canvasHeight);
    defer rl.unloadRenderTexture(canvas);

    var test_stack = try stack.fromFolder(allocator, "assets/cars/red/body", 9);
    defer test_stack.deinit(allocator);

    rl.setTextureFilter(canvas.texture, rl.TextureFilter.point);

    var test_rotation: f32 = 0;

    while (!rl.windowShouldClose()) {
        // update
        const dt = rl.getFrameTime();

        // const msg = try net.recv();
        // if (msg) |m| {
        //
        // }

        test_rotation += dt * 10;

        const fps = rl.getFPS();
        const fpsText = try std.fmt.bufPrintZ(&fps_buf, "fps {d}", .{fps});

        if (rl.isKeyPressed(rl.KeyboardKey.a)) {
            const smsg = "Hello from Zig!";
            try net.send(smsg, smsg.len);
        }

        // end update

        // draw game
        rl.beginTextureMode(canvas);
        rl.clearBackground(.black);

        test_stack.draw(.{ .x = 10, .y = 10 }, test_rotation);

        rl.endTextureMode();
        // end draw game

        // draw ui
        rl.beginDrawing();
        defer rl.endDrawing();

        rl.clearBackground(.black);

        rl.drawTexturePro(canvas.texture, .{
            .x = 0,
            .y = 0,
            .width = @floatFromInt(canvas.texture.width),
            .height = -@as(f32, @floatFromInt(canvas.texture.height)),
        }, .{ .x = 0, .y = 0, .width = @floatFromInt(screenWidth), .height = @floatFromInt(screenHeight) }, .{ .x = 0, .y = 0 }, 0, .white);

        rl.drawText(fpsText, 10, 10, 20, .white);
    }
}
