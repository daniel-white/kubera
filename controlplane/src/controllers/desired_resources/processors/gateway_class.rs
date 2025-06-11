use crate::api::v1alpha1::GatewayClassParameters;
use crate::constants::{GATEWAY_PARAMETERS_CRD_KIND, GROUP};
use crate::controllers::resources::Ref;
use gateway_api::apis::standard::gatewayclasses::{GatewayClass, GatewayClassStatus};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use k8s_openapi::chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use std::time::SystemTime;
use thiserror::Error;

pub fn process(
    gateway_class: &GatewayClass,
    gateway_class_parameters: &BTreeMap<Ref, GatewayClassParameters>,
) -> Result<GatewayClassParameters, GatewayClassProcessorError> {
    let parameters_ref = gateway_class.spec.parameters_ref.as_ref();

    let ref_ = match parameters_ref {
        Some(ref_param)
            if ref_param.kind == GATEWAY_PARAMETERS_CRD_KIND && ref_param.group == GROUP =>
        {
            Ref::new_builder()
                .namespace(ref_param.namespace.clone())
                .name(ref_param.name.clone())
                .build()
                .expect("Failed to build Ref")
        }
        Some(r) if r.kind != GATEWAY_PARAMETERS_CRD_KIND || r.group != GROUP => {
            return Err(GatewayClassProcessorError::InvalidParametersRefKind);
        }
        _ => {
            return Ok(GatewayClassParameters::default()); // TODO set sensible defaults
        }
    };

    gateway_class_parameters
        .get(&ref_)
        .ok_or(GatewayClassProcessorError::MissingParameters)
        .cloned()
}

#[derive(Error, Debug)]
pub enum GatewayClassProcessorError {
    #[error("Invalid parameters reference kind in GatewayClass")]
    InvalidParametersRefKind,

    #[error("Missing required parameters for GatewayClass")]
    MissingParameters,
}

pub fn into_status(
    r: &Result<GatewayClassParameters, GatewayClassProcessorError>,
) -> GatewayClassStatus {
    match r {
        Ok(params) => GatewayClassStatus {
            conditions: Some(vec![Condition {
                type_: "Ready".to_string(),
                status: "True".to_string(),
                last_transition_time: Time(DateTime::<Utc>::from(SystemTime::now())),
                reason: "".to_string(),
                message: "".to_string(),
                observed_generation: None,
            }]),
            ..Default::default()
        },
        Err(GatewayClassProcessorError::InvalidParametersRefKind) => GatewayClassStatus {
            conditions: Some(vec![Condition {
                type_: "NotReady".to_string(),
                status: "False".to_string(),
                last_transition_time: Time(DateTime::<Utc>::from(SystemTime::now())),
                reason: "InvalidParametersRefKind".to_string(),
                message: "Invalid parameters reference kind in GatewayClass".to_string(),
                observed_generation: None,
            }]),
            ..Default::default()
        },
        Err(GatewayClassProcessorError::MissingParameters) => GatewayClassStatus {
            conditions: Some(vec![Condition {
                type_: "Ready".to_string(),
                status: "False".to_string(),
                last_transition_time: Time(DateTime::<Utc>::from(SystemTime::now())),
                reason: "MissingParameters".to_string(),
                message: "Missing required parameters for GatewayClass".to_string(),
                observed_generation: None,
            }]),
            ..Default::default()
        },
    }
}
