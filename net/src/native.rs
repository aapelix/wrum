use wtransport::{ClientConfig, Endpoint};

pub struct NativeTransport {}

impl NativeTransport {
    pub async fn open(url: &str) -> anyhow::Result<Self> {
        let config = ClientConfig::default();
        let conn = Endpoint::client(config)?.connect(url).await?;

        Ok(Self {})
    }
}
