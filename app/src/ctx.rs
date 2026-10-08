use raylib::{
    RaylibHandle, RaylibThread,
    texture::{RaylibTexture2D, RenderTexture2D},
};

use crate::net::{Net, SERVER, Status};

pub struct Context {
    pub net: Net,
    pub last_status: Option<Status>,
    pub canvas: RenderTexture2D,

    pub rl: RaylibHandle,
    pub thread: RaylibThread,
}

impl Context {
    pub fn new(mut rl: RaylibHandle, thread: RaylibThread) -> Context {
        let canvas = rl.load_render_texture(&thread, 360, 180).unwrap();
        canvas.set_texture_filter(&thread, raylib::ffi::TextureFilter::TEXTURE_FILTER_POINT);

        Self {
            net: Net::new(SERVER).unwrap(),
            canvas,
            rl,
            thread,
            last_status: None,
        }
    }
}
