use axum::http;
use axum::response::IntoResponse;
use problemdetails::Problem;

pub async fn liveness_check() -> impl IntoResponse {
    Problem::from(http::StatusCode::OK)
        .with_title("Liveness Check")
        .with_detail("UP")
        .into_response()
}
