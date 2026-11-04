use std::collections::HashMap;

use raylib::{
    RaylibHandle, RaylibThread,
    drawing::{RaylibDraw, RaylibTextureModeExt},
    ffi::{Color, TextureFilter, Vector2},
    math::Rectangle,
    texture::{RaylibTexture2D, RenderTexture2D, Texture2D},
};

pub type AssetId = u16;

pub struct Assets {
    texs: HashMap<AssetId, Texture2D>,
    paths: HashMap<String, AssetId>,
    shadows: HashMap<String, RenderTexture2D>,
    next_id: AssetId,
}

impl Assets {
    pub fn new() -> Self {
        Self {
            texs: HashMap::new(),
            paths: HashMap::new(),
            shadows: HashMap::new(),
            next_id: 0,
        }
    }
    pub fn load(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        path: &str,
        shadow: Option<f32>,
    ) -> AssetId {
        if let Some(&id) = self.paths.get(path) {
            return id;
        }

        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).expect("too many assets");

        let tex = rl.load_texture(thread, path).unwrap();
        tex.set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_POINT);

        if let Some(sw) = shadow {
            let w = tex.width();
            let h = tex.height();
            let c = w / sw as i32;

            let mut shadow = rl.load_render_texture(&thread, w as u32, h as u32).unwrap();
            {
                let mut tm = rl.begin_texture_mode(&thread, &mut shadow);
                tm.clear_background(Color::BLANK);

                for i in 0..c {
                    tm.draw_texture_rec(
                        &tex,
                        Rectangle {
                            x: i as f32 * sw,
                            y: 0.,
                            width: sw,
                            height: h as f32,
                        },
                        Vector2::zero(),
                        Color::BLACK,
                    );
                }
            }
            self.shadows.insert(path.into(), shadow);
        }

        self.texs.insert(id, tex);
        self.paths.insert(path.to_owned(), id);

        id
    }
    pub fn get(&self, id: &AssetId) -> &Texture2D {
        self.texs.get(id).unwrap()
    }
    pub fn get_shadow(&self, id: &str) -> &RenderTexture2D {
        self.shadows.get(id).unwrap()
    }
}
