const std = @import("std");
const rlz = @import("raylib_zig");

pub fn build(b: *std.Build) !void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const raylib_dep = b.dependency("raylib_zig", .{
        .target = target,
        .optimize = optimize,
    });
    const raylib = raylib_dep.module("raylib");
    const raylib_artifact = raylib_dep.artifact("raylib");

    const exe_mod = b.createModule(.{
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
    });
    exe_mod.addImport("raylib", raylib);

    const run_step = b.step("run", "Run the app");

    if (target.query.os_tag == .emscripten) {
        const emsdk = rlz.emsdk;
        const wasm = b.addLibrary(.{
            .name = "game",
            .root_module = exe_mod,
        });

        const jsLibraryPath = b.path("lib.js");
        const jsLibraryString = try std.fmt.allocPrint(
            b.allocator,
            "--js-library={s}",
            .{jsLibraryPath.getPath(b)},
        );

        const install_dir: std.Build.InstallDir = .{ .custom = "web" };
        var emcc_flags = emsdk.emccDefaultFlags(b.allocator, .{ .optimize = optimize });
        emcc_flags.put(
            jsLibraryString,
            {},
        ) catch unreachable;
        emcc_flags.put("-sALLOW_MEMORY_GROWTH=1", {}) catch unreachable;
        emcc_flags.put("-sINITIAL_MEMORY=134217728", {}) catch unreachable;
        var emcc_settings = emsdk.emccDefaultSettings(b.allocator, .{ .optimize = optimize });
        emcc_settings.put("EXPORTED_RUNTIME_METHODS", "['requestFullscreen', 'ccall', 'cwrap']") catch unreachable;
        emcc_settings.put("EXPORTED_FUNCTIONS", "['_ws_on_message', '_ws_on_close', '_ws_on_open', '_main', '_malloc', '_free']") catch unreachable;

        const emcc_step = emsdk.emccStep(b, raylib_artifact, wasm, .{
            .optimize = optimize,
            .flags = emcc_flags,
            .settings = emcc_settings,
            .shell_file_path = emsdk.shell(raylib_dep.builder),
            .install_dir = install_dir,
            .embed_paths = &.{.{ .src_path = "assets", .virtual_path = "assets" }},
        });
        b.getInstallStep().dependOn(emcc_step);

        const html_filename = try std.fmt.allocPrint(b.allocator, "{s}.html", .{wasm.name});
        const emrun_step = emsdk.emrunStep(
            b,
            b.getInstallPath(install_dir, html_filename),
            &.{},
        );

        emrun_step.dependOn(emcc_step);
        run_step.dependOn(emrun_step);
    } else {
        const exe = b.addExecutable(.{
            .name = "game",
            .root_module = exe_mod,
        });
        b.installArtifact(exe);

        const run_cmd = b.addRunArtifact(exe);
        run_cmd.step.dependOn(b.getInstallStep());

        run_step.dependOn(&run_cmd.step);
    }
}
