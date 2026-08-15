use crate::authentication::{
    GuestRegistrationHasher, GuestRegistrationRateLimiter, reject_anonymous_users,
    reject_non_laboratory_users, reject_non_system_admins,
};
use crate::configuration::{
    ApplicationSettings, DatabaseSettings, FederationSettings, LabelPrintingSettings, PublicWebUrl,
    Settings,
};
use crate::file_storage::FileStorage;
use crate::routes::{
    accept_pairing, assign_asset_attachment, assign_inventory_item_attachment,
    batch_delete_inventory_items, batch_update_inventory_items, cancel_borrow_request,
    change_password, create_asset, create_asset_category, create_asset_parameter,
    create_borrow_request,
    create_guest_registration_code, create_inventory_items, create_label_printer, create_laboratory,
    create_location, create_pairing_code, create_trust, create_unit, create_user, delete_asset,
    delete_asset_category, delete_asset_parameter, delete_attachment, delete_file_upload,
    delete_inventory_item, delete_label_printer, delete_laboratory, delete_location, delete_unit,
    delete_user, download_attachment, enforce_guest_registration_rate_limit, get_asset,
    get_asset_category, get_asset_parameter, get_attachment, get_inventory_item,
    get_instance_identity, get_label_printer, get_label_printer_status, get_laboratory, get_location,
    get_unit, get_user, health_check, inbound_get, inbound_post, initialize_local_node,
    list_asset_attachments,
    list_asset_categories, list_asset_parameters, list_assets, list_audit_logs,
    list_borrow_requests, list_guest_links, list_inventory_item_attachments, list_inventory_items,
    list_label_printers, list_laboratories, list_laboratory_attachments, list_locations,
    list_my_borrow_requests,
    list_trusts, list_units, list_users, login, logout, me, merge_guest_link, merge_inventory_items,
    print_labels, proxy_get, proxy_post,
    register_guest, resolve_borrow_request, revoke_trust, split_inventory_item, update_asset,
    update_asset_category, update_asset_parameter, update_attachment, update_inventory_item,
    update_label_printer, update_laboratory, update_location, update_unit, update_user, upload_file,
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
        let rate_limit_namespace = configuration.database.database_name.clone();
        let public_web_url = configuration.public_web_url();

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
            configuration.file_storage,
            configuration.federation,
            configuration.label_printing,
            public_web_url,
            configuration.redis_uri,
            rate_limit_namespace,
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
    file_storage: crate::configuration::FileStorageSettings,
    federation: FederationSettings,
    label_printing: LabelPrintingSettings,
    public_web_url: PublicWebUrl,
    redis_uri: Secret<String>,
    rate_limit_namespace: String,
) -> Result<Server, anyhow::Error> {
    initialize_local_node(&db_pool).await?;
    let db_pool = Data::new(db_pool);
    let file_storage = Data::new(FileStorage::new(file_storage)?);
    let federation = Data::new(federation);
    let label_printing = Data::new(label_printing);
    let public_web_url = Data::new(public_web_url);
    let federation_client = Data::new(reqwest::Client::builder().tls_info(true).build()?);
    let registration_hasher = GuestRegistrationHasher::new(application.hmac_secret.clone());
    let registration_rate_limiter = GuestRegistrationRateLimiter::new(
        redis_uri.expose_secret(),
        &rate_limit_namespace,
        registration_hasher.clone(),
    )
    .await?;
    let secret_key = Key::derive_from(application.hmac_secret.expose_secret().as_bytes());
    let redis_store = RedisSessionStore::new(redis_uri.expose_secret()).await?;

    let server = build_server(
        listener,
        db_pool,
        file_storage,
        federation,
        federation_client,
        label_printing,
        public_web_url,
        Data::new(registration_hasher),
        Data::new(registration_rate_limiter),
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
    file_storage: Data<FileStorage>,
    federation: Data<FederationSettings>,
    federation_client: Data<reqwest::Client>,
    label_printing: Data<LabelPrintingSettings>,
    public_web_url: Data<PublicWebUrl>,
    registration_hasher: Data<GuestRegistrationHasher>,
    registration_rate_limiter: Data<GuestRegistrationRateLimiter>,
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
            .app_data(file_storage.clone())
            .app_data(federation.clone())
            .app_data(federation_client.clone())
            .app_data(label_printing.clone())
            .app_data(public_web_url.clone())
            .app_data(registration_hasher.clone())
            .app_data(registration_rate_limiter.clone())
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
                web::resource("/auth/guest-registration")
                    .wrap(from_fn(enforce_guest_registration_rate_limit))
                    .route(web::post().to(register_guest)),
            )
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
            // Every write tail is non-empty, so there is no bare-path variant.
            .route(
                "/federation/inbound/laboratories/{laboratory_id}/{tail:.*}",
                web::post().to(inbound_post),
            )
            .service(
                web::scope("")
                    .wrap(from_fn(reject_anonymous_users))
                    .configure(protected_routes),
            ),
    );
}

fn protected_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/auth/me", web::get().to(me))
        .route("/auth/password", web::patch().to(change_password))
        .route("/audit-logs", web::get().to(list_audit_logs))
        // Not under /local: system admins browse assets too, and that scope
        // rejects them.
        .route("/instance-identity", web::get().to(get_instance_identity))
        .route(
            "/federation/nodes/{remote_node_id}/laboratories/{remote_laboratory_id}",
            web::get().to(proxy_get),
        )
        .route(
            "/federation/nodes/{remote_node_id}/laboratories/{remote_laboratory_id}/{tail:.*}",
            web::get().to(proxy_get),
        )
        .route(
            "/federation/nodes/{remote_node_id}/laboratories/{remote_laboratory_id}/{tail:.*}",
            web::post().to(proxy_post),
        )
        .service(
            web::scope("/local")
                .wrap(from_fn(reject_non_laboratory_users))
                .configure(local_routes),
        )
        .service(
            web::scope("/admin")
                .wrap(from_fn(reject_non_system_admins))
                .configure(admin_routes),
        );
}

fn local_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/laboratory", web::get().to(get_laboratory))
        .route("/laboratory", web::patch().to(update_laboratory))
        .route("/users", web::post().to(create_user))
        .route("/users", web::get().to(list_users))
        .route("/users/{target_user_id}", web::get().to(get_user))
        .route("/users/{target_user_id}", web::patch().to(update_user))
        .route("/users/{target_user_id}", web::delete().to(delete_user))
        .route(
            "/guest-registration-codes",
            web::post().to(create_guest_registration_code),
        )
        .route(
            "/federation/pairing-codes",
            web::post().to(create_pairing_code),
        )
        .route("/federation/trusts", web::post().to(create_trust))
        .route("/federation/trusts", web::get().to(list_trusts))
        .route(
            "/federation/trusts/{trust_id}",
            web::delete().to(revoke_trust),
        )
        .route("/federation/guest-links", web::get().to(list_guest_links))
        .route(
            "/federation/guest-links/{link_id}/merge",
            web::post().to(merge_guest_link),
        )
        .route("/borrow-requests", web::get().to(list_borrow_requests))
        // Registered ahead of the parameterised routes so the literal wins.
        .route("/borrow-requests/mine", web::get().to(list_my_borrow_requests))
        .route(
            "/borrow-requests/{borrow_request_id}",
            web::patch().to(resolve_borrow_request),
        )
        .route(
            "/borrow-requests/{borrow_request_id}/cancel",
            web::post().to(cancel_borrow_request),
        )
        .route(
            "/inventory-items/{inventory_item_id}/borrow-requests",
            web::post().to(create_borrow_request),
        )
        .configure(laboratory_resource_routes);
}

fn admin_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/laboratories", web::post().to(create_laboratory))
        .route("/laboratories", web::get().to(list_laboratories))
        .route("/users", web::post().to(create_user))
        .route("/users", web::get().to(list_users))
        .route("/users/{target_user_id}", web::get().to(get_user))
        .route("/users/{target_user_id}", web::patch().to(update_user))
        .route("/users/{target_user_id}", web::delete().to(delete_user))
        .service(
            web::scope("/laboratories/{laboratory_id}")
                .route("", web::get().to(get_laboratory))
                .route("", web::patch().to(update_laboratory))
                .route("", web::delete().to(delete_laboratory))
                .configure(laboratory_resource_routes),
        );
}

fn laboratory_resource_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/asset-categories", web::get().to(list_asset_categories))
        .route("/asset-categories", web::post().to(create_asset_category))
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
        .route("/asset-parameters", web::get().to(list_asset_parameters))
        .route("/asset-parameters", web::post().to(create_asset_parameter))
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
        .route("/assets", web::get().to(list_assets))
        .route("/assets", web::post().to(create_asset))
        .route("/assets/{asset_id}", web::get().to(get_asset))
        .route("/assets/{asset_id}", web::patch().to(update_asset))
        .route("/assets/{asset_id}", web::delete().to(delete_asset))
        .route(
            "/assets/{asset_id}/attachments",
            web::post().to(assign_asset_attachment),
        )
        .route(
            "/assets/{asset_id}/attachments",
            web::get().to(list_asset_attachments),
        )
        .route(
            "/assets/{asset_id}/inventory-items",
            web::post().to(create_inventory_items),
        )
        .route("/label-printers", web::get().to(list_label_printers))
        .route("/label-printers", web::post().to(create_label_printer))
        .route(
            "/label-printers/{printer_id}",
            web::get().to(get_label_printer),
        )
        .route(
            "/label-printers/{printer_id}",
            web::patch().to(update_label_printer),
        )
        .route(
            "/label-printers/{printer_id}",
            web::delete().to(delete_label_printer),
        )
        .route(
            "/label-printers/{printer_id}/status",
            web::get().to(get_label_printer_status),
        )
        .route(
            "/label-printers/{printer_id}/print",
            web::post().to(print_labels),
        )
        .route("/inventory-items", web::get().to(list_inventory_items))
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
            web::post().to(assign_inventory_item_attachment),
        )
        .route(
            "/inventory-items/{inventory_item_id}/attachments",
            web::get().to(list_inventory_item_attachments),
        )
        .route("/units", web::get().to(list_units))
        .route("/units", web::post().to(create_unit))
        .route("/units/{unit_id}", web::get().to(get_unit))
        .route("/units/{unit_id}", web::patch().to(update_unit))
        .route("/units/{unit_id}", web::delete().to(delete_unit))
        .route("/locations", web::get().to(list_locations))
        .route("/locations", web::post().to(create_location))
        .route("/locations/{location_id}", web::get().to(get_location))
        .route("/locations/{location_id}", web::patch().to(update_location))
        .route(
            "/locations/{location_id}",
            web::delete().to(delete_location),
        )
        .route("/file-uploads", web::post().to(upload_file))
        .route(
            "/file-uploads/{upload_id}",
            web::delete().to(delete_file_upload),
        )
        .route("/attachments", web::get().to(list_laboratory_attachments))
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
        );
}
