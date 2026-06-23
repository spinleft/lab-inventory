use crate::authentication::reject_anonymous_users;
use crate::configuration::{ApplicationSettings, DatabaseSettings, Settings};
use crate::routes::{
    change_password, create_asset_category, create_laboratory, create_location, create_user,
    delete_asset_category, delete_laboratory, delete_location, delete_user, get_asset_category,
    get_laboratory, get_location, get_user, health_check, list_asset_categories, list_audit_logs,
    list_laboratories, list_locations, list_users, login, logout, me, update_asset_category,
    update_laboratory, update_location, update_user,
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

async fn run(
    listener: TcpListener,
    db_pool: PgPool,
    application: ApplicationSettings,
    redis_uri: Secret<String>,
) -> Result<Server, anyhow::Error> {
    let db_pool = Data::new(db_pool);
    let secret_key = Key::derive_from(application.hmac_secret.expose_secret().as_bytes());
    let redis_store = RedisSessionStore::new(redis_uri.expose_secret()).await?;

    let server = build_server(
        listener,
        db_pool,
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
                    .route("/auth/password", web::patch().to(change_password))
                    .route("/audit-logs", web::get().to(list_audit_logs))
                    .route("/laboratories", web::post().to(create_laboratory))
                    .route("/laboratories", web::get().to(list_laboratories))
                    .route(
                        "/laboratories/{laboratory_id}/asset-categories",
                        web::get().to(list_asset_categories),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/asset-categories",
                        web::post().to(create_asset_category),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/locations",
                        web::get().to(list_locations),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/locations",
                        web::post().to(create_location),
                    )
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
                    .route("/locations/{location_id}", web::get().to(get_location))
                    .route("/locations/{location_id}", web::patch().to(update_location))
                    .route(
                        "/locations/{location_id}",
                        web::delete().to(delete_location),
                    ),
            ),
    );
}
