use serde::Deserialize;

pub fn load_map(path: &str) -> anyhow::Result<LevelData> {
    let bytes = std::fs::read(path)?;

    let json = String::from_utf8(bytes)?;
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
    pub speed_pads: Vec<SpeedPad>,
    pub props: Vec<Prop>,
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

/// [x, y, w, h, rotation_degrees, tier]
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct SpeedPad(pub f32, pub f32, pub f32, pub f32, pub f32, pub u32);

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PropKind {
    Fence,
    FenceEnd,
    Tires,
}

#[derive(Debug, Clone, Copy)]
pub struct Prop {
    pub x: f32,
    pub y: f32,
    pub rotation: f32,
    pub kind: PropKind,
}
impl<'de> Deserialize<'de> for Prop {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (x, y, rotation, kind) = <(f32, f32, f32, PropKind)>::deserialize(deserializer)?;

        Ok(Self {
            x,
            y,
            rotation,
            kind,
        })
    }
}
