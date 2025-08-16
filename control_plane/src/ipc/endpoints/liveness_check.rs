use axum::http;
use axum::response::IntoResponse;
use problemdetails::Problem;
use tracing::instrument;
use vg_core::instrumentation::trace_id;

#[instrument(name = "ipc::liveness_check")]

pub async fn liveness_check() -> impl IntoResponse {
    let mut problem = Problem::from(http::StatusCode::OK)
        .with_value("status", http::StatusCode::OK.as_u16())
        .with_title("Liveness Check")
        .with_detail("UP");

    if let Some(trace_id) = trace_id() {
        problem = problem.with_instance(trace_id);
    }

    problem.into_response()
}
