use crate::kubernetes::KubeClientCell;
use async_trait::async_trait;
use axum_health::{HealthDetail, HealthIndicator};
use kube::Api;
use kube::api::ListParams;
use vg_api::v1alpha1::GatewayClassParameters;
use vg_core::sync::signal::Receiver;
use std::ops::Deref;

pub struct KubernetesApiHealthIndicator(Receiver<KubeClientCell>);

impl KubernetesApiHealthIndicator {
    pub fn new(kube_client: &Receiver<KubeClientCell>) -> Self {
        Self(kube_client.clone())
    }
}

#[async_trait]
impl HealthIndicator for KubernetesApiHealthIndicator {
    fn name(&self) -> String {
        "KubernetesAPI".to_string()
    }

    async fn details(&self) -> HealthDetail {
        if let Some(kube_client) = self.0.get().await {
            let api = Api::<GatewayClassParameters>::all(kube_client.deref().clone());
            match api.list(&ListParams::default()).await {
                Ok(_) => HealthDetail::up(),
                Err(e) => {
                    let mut health = HealthDetail::down();
                    health.with_detail("error".to_string(), e.to_string());
                    health
                }
            }
        } else {
            let mut heath = HealthDetail::down();
            heath.with_detail("error".to_string(), "Kube client not available".to_string());
            heath
        }
    }
}
