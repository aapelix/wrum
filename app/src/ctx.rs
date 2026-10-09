use raylib::{
    RaylibHandle, RaylibThread,
    texture::{RaylibTexture2D, RenderTexture2D},
};

use crate::net::{Net, SERVER, Status};

pub struct State {
    pub net: Net,
    pub last_status: Option<Status>,
    pub canvas: RenderTexture2D,
}

pub struct Context<'a> {
    pub net: &'a mut Net,
    pub last_status: &'a mut Option<Status>,
    pub canvas: &'a mut RenderTexture2D,
    pub rl: &'a mut RaylibHandle,
    pub thread: &'a RaylibThread,
}

impl State {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> Self {
        let canvas = rl.load_render_texture(&thread, 360, 180).unwrap();
        canvas.set_texture_filter(&thread, raylib::ffi::TextureFilter::TEXTURE_FILTER_POINT);
        Self {
            net: Net::new(SERVER).unwrap(),
            last_status: None,
            canvas,
        }
    }

    pub fn ctx<'a>(
        &'a mut self,
        rl: &'a mut RaylibHandle,
        thread: &'a RaylibThread,
    ) -> Context<'a> {
        Context {
            net: &mut self.net,
            last_status: &mut self.last_status,
            canvas: &mut self.canvas,
            rl,
            thread,
        }
    }
}
