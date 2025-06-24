pub const MANAGED_BY_LABEL: &str = "app.kubernetes.io/managed-by";
pub const MANAGED_BY_VALUE: &str = "kubera-controlplane";
pub const MANAGED_BY_LABEL_QUERY: &str = "app.kubernetes.io/managed-by=kubera-controlplane";

pub const PART_OF_LABEL: &str = "app.kubernetes.io/part-of";

pub const CONFIGMAP_ROLE_LABEL: &str = "kubera.whitefamily.in/configmap-role";

pub const CONFIGMAP_ROLE_GATEWAY_CONFIG: &str = "gateway-configuration";

pub const GROUP: &str = "kubera.whitefamily.in";
pub const GATEWAY_CLASS_PARAMETERS_CRD_KIND: &str = "GatewayClassParameters";
pub const GATEWAY_PARAMETERS_CRD_KIND: &str = "GatewayParameters";

pub const GATEWAY_CLASS_CONTROLLER_NAME: &str = "kubera.whitefamily.in/controlplane";
