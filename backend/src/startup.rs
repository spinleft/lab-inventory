use crate::attachment_storage::AttachmentStorage;
use crate::authentication::reject_anonymous_users;
use crate::configuration::{ApplicationSettings, DatabaseSettings, FederationSettings, Settings};
use crate::routes::{
    accept_pairing, batch_delete_inventory_items, batch_update_inventory_items, change_password,
    create_asset, create_asset_attachment, create_asset_category, create_asset_parameter,
    create_inventory_item_attachment, create_inventory_items, create_laboratory, create_location,
    create_pairing_code, create_trust, create_unit, create_user, delete_asset,
    delete_asset_category, delete_asset_parameter, delete_attachment, delete_attachment_upload,
    delete_inventory_item, delete_laboratory, delete_location, delete_unit, delete_user,
    download_attachment, get_asset, get_asset_category, get_asset_parameter, get_attachment,
    get_inventory_item, get_laboratory, get_location, get_unit, get_user, health_check,
    inbound_get, initialize_local_node, list_asset_attachments, list_asset_categories,
    list_asset_parameters, list_assets, list_audit_logs, list_guest_links,
    list_inventory_item_attachments, list_inventory_items, list_laboratories,
    list_laboratory_attachments, list_locations, list_trusts, list_units, list_users, login,
    logout, me, merge_guest_link, merge_inventory_items, proxy_get, revoke_trust,
    split_inventory_item, update_asset, update_asset_category, update_asset_parameter,
    update_attachment, update_inventory_item, update_laboratory, update_location, update_unit,
    update_user, upload_attachment,
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
            configuration.attachment_storage,
            configuration.federation,
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
    attachment_storage: crate::configuration::AttachmentStorageSettings,
    federation: FederationSettings,
    redis_uri: Secret<String>,
) -> Result<Server, anyhow::Error> {
    initialize_local_node(&db_pool, &federation).await?;
    let db_pool = Data::new(db_pool);
    let attachment_storage = Data::new(AttachmentStorage::new(attachment_storage)?);
    let federation = Data::new(federation);
    let federation_client = Data::new(reqwest::Client::builder().tls_info(true).build()?);
    let secret_key = Key::derive_from(application.hmac_secret.expose_secret().as_bytes());
    let redis_store = RedisSessionStore::new(redis_uri.expose_secret()).await?;

    let server = build_server(
        listener,
        db_pool,
        attachment_storage,
        federation,
        federation_client,
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
    attachment_storage: Data<AttachmentStorage>,
    federation: Data<FederationSettings>,
    federation_client: Data<reqwest::Client>,
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
            .app_data(attachment_storage.clone())
            .app_data(federation.clone())
            .app_data(federation_client.clone())
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
            .route(
                "/federation/inbound/pairing/accept",
                web::post().to(accept_pairing),
            )
            .route(
                "/federation/inbound/laboratories/{laboratory_id}",
                web::get().to(inbound_get),
            )
            .route(
                "/federation/inbound/laboratories/{laboratory_id}/{tail:.*}",
                web::get().to(inbound_get),
            )
            .service(
                web::scope("")
                    .wrap(from_fn(reject_anonymous_users))
                    .route("/auth/me", web::get().to(me))
                    .route("/auth/password", web::patch().to(change_password))
                    .route("/audit-logs", web::get().to(list_audit_logs))
                    .route("/units", web::get().to(list_units))
                    .route("/units", web::post().to(create_unit))
                    .route(
                        "/federation/nodes/{remote_node_id}/laboratories/{remote_laboratory_id}",
                        web::get().to(proxy_get),
                    )
                    .route(
                        "/federation/nodes/{remote_node_id}/laboratories/{remote_laboratory_id}/{tail:.*}",
                        web::get().to(proxy_get),
                    )
                    .route("/laboratories", web::post().to(create_laboratory))
                    .route("/laboratories", web::get().to(list_laboratories))
                    .route(
                        "/laboratories/{laboratory_id}/federation/pairing-codes",
                        web::post().to(create_pairing_code),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/federation/trusts",
                        web::post().to(create_trust),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/federation/trusts",
                        web::get().to(list_trusts),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/federation/trusts/{trust_id}",
                        web::delete().to(revoke_trust),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/federation/guest-links",
                        web::get().to(list_guest_links),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/federation/guest-links/{link_id}/merge",
                        web::post().to(merge_guest_link),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/asset-categories",
                        web::get().to(list_asset_categories),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/asset-categories",
                        web::post().to(create_asset_category),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/asset-parameters",
                        web::get().to(list_asset_parameters),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/asset-parameters",
                        web::post().to(create_asset_parameter),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/assets",
                        web::get().to(list_assets),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/assets",
                        web::post().to(create_asset),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/attachment-uploads",
                        web::post().to(upload_attachment),
                    )
                    .route(
                        "/attachment-uploads/{upload_id}",
                        web::delete().to(delete_attachment_upload),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/attachments",
                        web::get().to(list_laboratory_attachments),
                    )
                    .route(
                        "/laboratories/{laboratory_id}/inventory-items",
                        web::get().to(list_inventory_items),
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
                    .route("/units/{unit_id}", web::get().to(get_unit))
                    .route("/units/{unit_id}", web::patch().to(update_unit))
                    .route("/units/{unit_id}", web::delete().to(delete_unit))
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
                    .route(
                        "/asset-parameters/{parameter_id}",
                        web::get().to(get_asset_parameter),
                    )
                    .route(
                        "/asset-parameters/{parameter_id}",
                        web::patch().to(update_asset_parameter),
                    )
                    .route(
                        "/asset-parameters/{parameter_id}",
                        web::delete().to(delete_asset_parameter),
                    )
                    .route("/assets/{asset_id}", web::get().to(get_asset))
                    .route("/assets/{asset_id}", web::patch().to(update_asset))
                    .route("/assets/{asset_id}", web::delete().to(delete_asset))
                    .route(
                        "/assets/{asset_id}/attachments",
                        web::post().to(create_asset_attachment),
                    )
                    .route(
                        "/assets/{asset_id}/attachments",
                        web::get().to(list_asset_attachments),
                    )
                    .route(
                        "/assets/{asset_id}/inventory-items",
                        web::post().to(create_inventory_items),
                    )
                    .route(
                        "/inventory-items/batch",
                        web::patch().to(batch_update_inventory_items),
                    )
                    .route(
                        "/inventory-items/batch-delete",
                        web::post().to(batch_delete_inventory_items),
                    )
                    .route(
                        "/inventory-items/merge",
                        web::post().to(merge_inventory_items),
                    )
                    .route(
                        "/inventory-items/{inventory_item_id}/split",
                        web::post().to(split_inventory_item),
                    )
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
                        "/inventory-items/{inventory_item_id}/attachments",
                        web::post().to(create_inventory_item_attachment),
                    )
                    .route(
                        "/inventory-items/{inventory_item_id}/attachments",
                        web::get().to(list_inventory_item_attachments),
                    )
                    .route(
                        "/attachments/{attachment_id}",
                        web::get().to(get_attachment),
                    )
                    .route(
                        "/attachments/{attachment_id}",
                        web::patch().to(update_attachment),
                    )
                    .route(
                        "/attachments/{attachment_id}",
                        web::delete().to(delete_attachment),
                    )
                    .route(
                        "/attachments/{attachment_id}/download",
                        web::get().to(download_attachment),
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
