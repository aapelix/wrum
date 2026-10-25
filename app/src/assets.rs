use std::collections::HashMap;

use raylib::{
    RaylibHandle, RaylibThread,
    ffi::TextureFilter,
    texture::{RaylibTexture2D, Texture2D},
};

pub type AssetId = u16;

pub struct Assets {
    texs: HashMap<AssetId, Texture2D>,
    paths: HashMap<String, AssetId>,
    next_id: AssetId,
}

impl Assets {
    pub fn new() -> Self {
        Self {
            texs: HashMap::new(),
            paths: HashMap::new(),
            next_id: 0,
        }
    }
    pub fn load(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread, path: &str) -> AssetId {
        if let Some(&id) = self.paths.get(path) {
            return id;
        }

        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).expect("too many assets");

        let tex = rl.load_texture(thread, path).unwrap();
        tex.set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_POINT);

        self.texs.insert(id, tex);
        self.paths.insert(path.to_owned(), id);

        id
    }
    pub fn get(&self, id: &AssetId) -> &Texture2D {
        self.texs.get(id).unwrap()
    }
}
