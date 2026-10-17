use serde::Deserialize;

pub fn load_map(path: &str) -> anyhow::Result<MapJson> {
    let json = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}

#[derive(Deserialize)]
pub struct MapJson {
    pub layers: Vec<LayerJson>,
    pub tilesets: Vec<TilesetJson>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum LayerJson {
    #[serde(rename = "tilelayer")]
    TileLayer(TileLayerJson),
    #[serde(rename = "objectgroup")]
    ObjectLayer(ObjectLayerJson),
}
#[derive(Debug, Deserialize)]
pub struct TileLayerJson {
    pub data: Vec<u32>,
    pub height: u32,
    pub id: u32,
    pub name: String,
    pub opacity: f32,
    pub visible: bool,
    pub width: u32,
    pub x: i32,
    pub y: i32,
}
#[derive(Debug, Deserialize)]
pub struct ObjectLayerJson {
    pub draworder: String,
    pub id: u32,
    pub name: String,
    pub objects: Vec<ObjectJson>,
    pub opacity: f32,
    pub visible: bool,
    pub x: i32,
    pub y: i32,
}
#[derive(Debug, Deserialize)]
pub struct ObjectJson {
    pub height: f32,
    pub id: u32,
    pub name: String,
    pub opacity: f32,
    pub rotation: f32,
    pub visible: bool,
    pub width: f32,
    pub x: f32,
    pub y: f32,
    #[serde(flatten)]
    pub kind: ObjectKindJson,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ObjectKindJson {
    #[serde(rename = "fence")]
    Fence { point: bool },
    #[serde(rename = "wall")]
    Wall,
    #[serde(rename = "water")]
    Water,
    #[serde(rename = "spawn")]
    Spawn,
    #[serde(rename = "grass")]
    Grass { polygon: Vec<Polygon> },
    #[serde(rename = "road")]
    Road { polygon: Vec<Polygon> },
    #[serde(rename = "check")]
    Check { properties: Vec<PropJson> },
}

#[derive(Deserialize, Debug)]
pub struct PropJson {
    pub name: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct Polygon {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Deserialize)]
pub struct TilesetJson {
    pub columns: u32,
    pub firstgid: u32,
    pub image: String,
    pub imageheight: u32,
    pub imagewidth: u32,
    pub margin: u32,
    pub name: String,
    pub spacing: u32,
    pub tilecount: u32,
    pub tileheight: u32,
    pub tilewidth: u32,
}
