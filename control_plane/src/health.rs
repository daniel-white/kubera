use crate::kubernetes::KubeClientCell;
use async_trait::async_trait;
use axum_health::{HealthDetail, HealthIndicator};
use kube::Api;
use kube::api::ListParams;
use kubera_api::v1alpha1::GatewayClassParameters;
use kubera_core::sync::signal::Receiver;

pub struct KubernetesApiHealthIndicator(Receiver<Option<KubeClientCell>>);

impl KubernetesApiHealthIndicator {
    pub fn new(kube_client: &Receiver<Option<KubeClientCell>>) -> Self {
        Self(kube_client.clone())
    }
}

#[async_trait]
impl HealthIndicator for KubernetesApiHealthIndicator {
    fn name(&self) -> String {
        "KubernetesAPI".to_string()
    }

    async fn details(&self) -> HealthDetail {
        let kube_client = self.0.current();
        let kube_client = if let Some(kube_client) = kube_client.as_ref() {
            kube_client.clone()
        } else {
            let mut heath = HealthDetail::down();
            heath.with_detail("error".to_string(), "Kube client not available".to_string());
            return heath;
        };

        let api = Api::<GatewayClassParameters>::all(kube_client.into());

        match api.list(&ListParams::default()).await {
            Ok(_) => HealthDetail::up(),
            Err(e) => {
                let mut health = HealthDetail::down();
                health.with_detail("error".to_string(), e.to_string());
                health
            }
        }
    }
}
