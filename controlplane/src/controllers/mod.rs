mod sources;

use anyhow::Result;
use derive_builder::Builder;
use getset::Getters;
use kube::Client;
use tokio::task::JoinSet;

pub async fn run() -> Result<()> {
    let mut join_set = JoinSet::new();
    let client = Client::try_default().await?;

    sources::spawn_sources(&mut join_set, &client).await?;

    join_set.join_all().await;

    Ok(())
}

#[derive(Builder, Getters, Clone, Debug, PartialEq, Eq, Hash)]
#[builder(setter(into))]
pub struct Ref {
    #[getset(get = "pub")]
    name: String,
    #[getset(get = "pub")]
    namespace: Option<String>,
}

impl Ref {
    pub fn new_builder() -> RefBuilder {
        RefBuilder::default()
    }
}
