use serde::Deserialize;

pub fn load_map(path: &str) -> anyhow::Result<LevelData> {
    let json = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct LevelData {
    pub checkpoints: Vec<Checkpoint>,
    pub water: Vec<Water>,
    pub spawns: Vec<Spawn>,
    pub colliders: Vec<Collider>,
    pub jump_pads: Vec<JumpPad>,
}

/// [x, y, w, h, rotation_degrees, index]
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Checkpoint(pub f32, pub f32, pub f32, pub f32, pub f32, pub u32);

/// [x, y, w, h]
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Water(pub f32, pub f32, pub f32, pub f32);

/// [x, y, r]
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Spawn(pub f32, pub f32, pub f32);

/// [x, y, w, h, rotation_degrees]
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Collider(pub f32, pub f32, pub f32, pub f32, pub f32);

/// [x, y, w, h, rotation_degrees, power]
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct JumpPad(pub f32, pub f32, pub f32, pub f32, pub f32, pub f32);
