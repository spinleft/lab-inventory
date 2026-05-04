use crate::authentication::reject_anonymous_users;
use crate::configuration::{ApplicationSettings, DatabaseSettings, Settings};
use crate::routes::{
    adjust_inventory_item, allocate_inventory_item, approve_borrow_request, cancel_borrow_request,
    create_asset, create_asset_category, create_attachment, create_borrow_request,
    create_inventory_item, create_laboratory, create_location, create_maintenance_record,
    create_maintenance_schedule, create_user, delete_asset, delete_asset_category,
    delete_attachment, delete_inventory_item, delete_laboratory, delete_location,
    delete_maintenance_record, delete_maintenance_schedule, delete_user, export_assets_csv,
    export_borrow_requests_csv, export_inventory_items_csv, export_maintenance_records_csv,
    get_asset, get_asset_category, get_borrow_request, get_inventory_item, get_laboratory,
    get_location, get_maintenance_record, get_user, health_check, list_asset_categories,
    list_assets, list_attachments, list_audit_logs, list_borrow_request_alerts,
    list_borrow_requests, list_inventory_items, list_laboratories, list_locations,
    list_maintenance_alerts, list_maintenance_records, list_maintenance_schedules,
    list_stock_alerts, list_units, list_users, login, logout, mark_borrow_request_borrowed, me,
    move_inventory_item, reject_borrow_request, release_inventory_item_allocation,
    return_borrow_request, stocktake_inventory_item, update_asset, update_asset_category,
    update_inventory_item, update_laboratory, update_location, update_maintenance_record,
    update_maintenance_schedule, update_user,
};
use actix_cors::Cors;
use actix_session::SessionMiddleware;
use actix_session::config::PersistentSession;
use actix_session::storage::RedisSessionStore;
use actix_web::cookie::time::Duration;
use actix_web::cookie::{Key, SameSite};
use actix_web::dev::Server;
use actix_web::http::header;
use actix_web::middleware::from_fn;
use actix_web::web::Data;
use actix_web::{App, HttpServer, web};
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, anyhow::Error> {
        let connection_pool = get_connection_pool(&configuration.database);

        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run(
            listener,
            connection_pool,
            configuration.application,
            configuration.redis_uri,
        )
        .await?;

        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub fn get_connection_pool(configuration: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new().connect_lazy_with(configuration.connect_options())
}

pub struct ApplicationBaseUrl(pub String);

async fn run(
    listener: TcpListener,
    db_pool: PgPool,
    application: ApplicationSettings,
    redis_uri: Secret<String>,
) -> Result<Server, anyhow::Error> {
    let db_pool = Data::new(db_pool);
    let base_url = Data::new(ApplicationBaseUrl(application.base_url));
    let secret_key = Key::derive_from(application.hmac_secret.expose_secret().as_bytes());
    let redis_store = RedisSessionStore::new(redis_uri.expose_secret()).await?;

    let server = build_server(
        listener,
        db_pool,
        base_url,
        secret_key,
        application.cookie_secure,
        application.cors_allowed_origins,
        redis_store,
    )?;

    Ok(server)
}

fn build_server(
    listener: TcpListener,
    db_pool: Data<PgPool>,
    base_url: Data<ApplicationBaseUrl>,
    secret_key: Key,
    cookie_secure: bool,
    cors_allowed_origins: Vec<String>,
    redis_store: RedisSessionStore,
) -> Result<Server, anyhow::Error> {
    let server = HttpServer::new(move || {
        App::new()
            .wrap(build_cors(&cors_allowed_origins))
            .wrap(build_session(
                redis_store.clone(),
                secret_key.clone(),
                cookie_secure,
            ))
            .wrap(TracingLogger::default())
            .configure(api_routes)
            .app_data(db_pool.clone())
            .app_data(base_url.clone())
    })
    .listen(listener)?
    .run();
    Ok(server)
}

fn build_cors(cors_allowed_origins: &[String]) -> Cors {
    let mut cors = Cors::default()
        .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"])
        .allowed_headers(vec![
            header::AUTHORIZATION,
            header::ACCEPT,
            header::CONTENT_TYPE,
        ])
        .supports_credentials()
        .max_age(3600);
    for origin in cors_allowed_origins {
        cors = cors.allowed_origin(origin);
    }
    cors
}

fn build_session(
    redis_store: RedisSessionStore,
    secret_key: Key,
    cookie_secure: bool,
) -> SessionMiddleware<RedisSessionStore> {
    SessionMiddleware::builder(redis_store, secret_key)
        .cookie_name("session_id".to_string())
        .cookie_secure(cookie_secure)
        .cookie_http_only(true)
        .cookie_same_site(SameSite::Lax)
        .cookie_path("/".to_string())
        .session_lifecycle(PersistentSession::default().session_ttl(Duration::hours(24)))
        .build()
}

fn api_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .route("/health_check", web::get().to(health_check))
            .route("/auth/login", web::post().to(login))
            .route("/auth/logout", web::post().to(logout))
            .service(
                web::scope("")
                    .wrap(from_fn(reject_anonymous_users))
                    .route("/auth/me", web::get().to(me))
                    .route("/audit-logs", web::get().to(list_audit_logs))
                    .route("/units", web::get().to(list_units))
                    .route("/laboratories", web::post().to(create_laboratory))
                    .route("/laboratories", web::get().to(list_laboratories))
                    .route(
                        "/laboratories/{laboratory_id}",
                        web::get().to(get_laboratory),
                    )
                    .route(
                        "/laboratories/{laboratory_id}",
                        web::patch().to(update_laboratory),
                    )
                    .route(
                        "/laboratories/{laboratory_id}",
                        web::delete().to(delete_laboratory),
                    )
                    .route("/users", web::post().to(create_user))
                    .route("/users", web::get().to(list_users))
                    .route("/users/{target_user_id}", web::get().to(get_user))
                    .route("/users/{target_user_id}", web::patch().to(update_user))
                    .route("/users/{target_user_id}", web::delete().to(delete_user))
                    .route("/asset-categories", web::post().to(create_asset_category))
                    .route("/asset-categories", web::get().to(list_asset_categories))
                    .route(
                        "/asset-categories/{category_id}",
                        web::get().to(get_asset_category),
                    )
                    .route(
                        "/asset-categories/{category_id}",
                        web::patch().to(update_asset_category),
                    )
                    .route(
                        "/asset-categories/{category_id}",
                        web::delete().to(delete_asset_category),
                    )
                    .route("/locations", web::post().to(create_location))
                    .route("/locations", web::get().to(list_locations))
                    .route("/locations/{location_id}", web::get().to(get_location))
                    .route("/locations/{location_id}", web::patch().to(update_location))
                    .route(
                        "/locations/{location_id}",
                        web::delete().to(delete_location),
                    )
                    .route("/assets", web::post().to(create_asset))
                    .route("/assets", web::get().to(list_assets))
                    .route("/assets/{asset_id}", web::get().to(get_asset))
                    .route("/assets/{asset_id}", web::patch().to(update_asset))
                    .route("/assets/{asset_id}", web::delete().to(delete_asset))
                    .route("/attachments", web::post().to(create_attachment))
                    .route("/attachments", web::get().to(list_attachments))
                    .route(
                        "/attachments/{attachment_id}",
                        web::delete().to(delete_attachment),
                    )
                    .route(
                        "/maintenance-records",
                        web::post().to(create_maintenance_record),
                    )
                    .route(
                        "/maintenance-records",
                        web::get().to(list_maintenance_records),
                    )
                    .route(
                        "/maintenance-records/{maintenance_record_id}",
                        web::get().to(get_maintenance_record),
                    )
                    .route(
                        "/maintenance-records/{maintenance_record_id}",
                        web::patch().to(update_maintenance_record),
                    )
                    .route(
                        "/maintenance-records/{maintenance_record_id}",
                        web::delete().to(delete_maintenance_record),
                    )
                    .route(
                        "/maintenance-schedules",
                        web::post().to(create_maintenance_schedule),
                    )
                    .route(
                        "/maintenance-schedules",
                        web::get().to(list_maintenance_schedules),
                    )
                    .route(
                        "/maintenance-schedules/{maintenance_schedule_id}",
                        web::patch().to(update_maintenance_schedule),
                    )
                    .route(
                        "/maintenance-schedules/{maintenance_schedule_id}",
                        web::delete().to(delete_maintenance_schedule),
                    )
                    .route(
                        "/maintenance-alerts",
                        web::get().to(list_maintenance_alerts),
                    )
                    .route("/exports/assets.csv", web::get().to(export_assets_csv))
                    .route(
                        "/exports/inventory-items.csv",
                        web::get().to(export_inventory_items_csv),
                    )
                    .route(
                        "/exports/borrow-requests.csv",
                        web::get().to(export_borrow_requests_csv),
                    )
                    .route(
                        "/exports/maintenance-records.csv",
                        web::get().to(export_maintenance_records_csv),
                    )
                    .route("/stock-alerts", web::get().to(list_stock_alerts))
                    .route("/inventory-items", web::post().to(create_inventory_item))
                    .route("/inventory-items", web::get().to(list_inventory_items))
                    .route(
                        "/inventory-items/{inventory_item_id}",
                        web::get().to(get_inventory_item),
                    )
                    .route(
                        "/inventory-items/{inventory_item_id}",
                        web::patch().to(update_inventory_item),
                    )
                    .route(
                        "/inventory-items/{inventory_item_id}",
                        web::delete().to(delete_inventory_item),
                    )
                    .route(
                        "/inventory-items/{inventory_item_id}/adjust",
                        web::post().to(adjust_inventory_item),
                    )
                    .route(
                        "/inventory-items/{inventory_item_id}/move",
                        web::post().to(move_inventory_item),
                    )
                    .route(
                        "/inventory-items/{inventory_item_id}/stocktake",
                        web::post().to(stocktake_inventory_item),
                    )
                    .route(
                        "/inventory-items/{inventory_item_id}/allocate",
                        web::post().to(allocate_inventory_item),
                    )
                    .route(
                        "/inventory-items/{inventory_item_id}/release-allocation",
                        web::post().to(release_inventory_item_allocation),
                    )
                    .route("/borrow-requests", web::post().to(create_borrow_request))
                    .route("/borrow-requests", web::get().to(list_borrow_requests))
                    .route(
                        "/borrow-request-alerts",
                        web::get().to(list_borrow_request_alerts),
                    )
                    .route(
                        "/borrow-requests/{borrow_request_id}",
                        web::get().to(get_borrow_request),
                    )
                    .route(
                        "/borrow-requests/{borrow_request_id}/approve",
                        web::post().to(approve_borrow_request),
                    )
                    .route(
                        "/borrow-requests/{borrow_request_id}/reject",
                        web::post().to(reject_borrow_request),
                    )
                    .route(
                        "/borrow-requests/{borrow_request_id}/cancel",
                        web::post().to(cancel_borrow_request),
                    )
                    .route(
                        "/borrow-requests/{borrow_request_id}/mark-borrowed",
                        web::post().to(mark_borrow_request_borrowed),
                    )
                    .route(
                        "/borrow-requests/{borrow_request_id}/return",
                        web::post().to(return_borrow_request),
                    ),
            ),
    );
}
