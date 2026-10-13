use raylib::{
    RaylibHandle, RaylibThread,
    texture::{RaylibTexture2D, RenderTexture2D},
};

use crate::net::Net;

pub struct State {
    pub net: Net,
    pub canvas: RenderTexture2D,
}

pub struct Context<'a> {
    pub net: &'a mut Net,
    pub canvas: &'a mut RenderTexture2D,
    pub rl: &'a mut RaylibHandle,
    pub thread: &'a RaylibThread,
}

impl State {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> anyhow::Result<Self> {
        let canvas = rl.load_render_texture(&thread, 360, 180).unwrap();
        canvas.set_texture_filter(&thread, raylib::ffi::TextureFilter::TEXTURE_FILTER_POINT);
        Ok(Self {
            net: Net::connect("https://127.0.0.1:7778")?,
            canvas,
        })
    }
    pub fn ctx<'a>(
        &'a mut self,
        rl: &'a mut RaylibHandle,
        thread: &'a RaylibThread,
    ) -> Context<'a> {
        Context {
            net: &mut self.net,
            canvas: &mut self.canvas,
            rl,
            thread,
        }
    }
}
