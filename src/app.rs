use raylib::{RaylibHandle, RaylibThread, texture::RenderTexture2D};

pub struct App {
    canvas: RenderTexture2D,
    thread: RaylibThread,
    rl: RaylibHandle,
}
