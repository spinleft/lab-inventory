use actix_web::HttpResponse;

pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({ "status": "ok" }))
}
