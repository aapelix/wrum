use raylib::{
    RaylibHandle, RaylibThread,
    texture::{RaylibTexture2D, RenderTexture2D},
};

use crate::{assets::Assets, net::Net};

pub struct State {
    pub net: Net,
    pub canvas: RenderTexture2D,
    pub assets: Assets,
}

pub struct Context<'a> {
    pub net: &'a mut Net,
    pub canvas: &'a mut RenderTexture2D,
    pub assets: &'a mut Assets,
    pub rl: &'a mut RaylibHandle,
    pub thread: &'a RaylibThread,
}

impl State {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> anyhow::Result<Self> {
        let canvas = rl.load_render_texture(&thread, 320, 180).unwrap();
        canvas.set_texture_filter(&thread, raylib::ffi::TextureFilter::TEXTURE_FILTER_POINT);
        Ok(Self {
            net: Net::connect(option_env!("WRUM_SERVER_URL").unwrap_or("ws://localhost:8080/ws"))?,
            canvas,
            assets: Assets::new(),
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
            assets: &mut self.assets,
            rl,
            thread,
        }
    }
}
