use secrecy::{ExposeSecret, Secret};
use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::postgres::{PgConnectOptions, PgSslMode};
use std::convert::{TryFrom, TryInto};

#[derive(serde::Deserialize, Clone)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
    pub file_storage: FileStorageSettings,
    pub federation: FederationSettings,
    #[serde(default)]
    pub label_printing: LabelPrintingSettings,
    pub redis_uri: Secret<String>,
}

#[derive(serde::Deserialize, Clone)]
pub struct ApplicationSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub base_url: String,
    /// Origin the browser app is served from.
    ///
    /// This is what QR codes point at, so it has to be the SPA's origin rather
    /// than the API's. When unset it falls back to the federation public base
    /// URL, which is correct for deployments that serve both from one host.
    #[serde(default)]
    pub public_web_url: Option<String>,
    pub hmac_secret: Secret<String>,
    pub cookie_secure: bool,
    pub enable_federation: bool,
    pub cors_allowed_origins: Vec<String>,
}

#[derive(serde::Deserialize, Clone, Default)]
pub struct LabelPrintingSettings {
    /// Whether a printer may be registered on a loopback address.
    ///
    /// Off in production, where loopback would let a printer registration point
    /// back at this server. Tests and local development turn it on so a fake
    /// printer can listen on 127.0.0.1.
    #[serde(default)]
    pub allow_loopback: bool,
}

/// The resolved origin QR codes link to, worked out once at startup.
#[derive(Clone, Debug)]
pub struct PublicWebUrl(pub String);

impl Settings {
    /// The SPA origin, falling back to the federation public base URL.
    pub fn public_web_url(&self) -> PublicWebUrl {
        let url = self
            .application
            .public_web_url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&self.federation.public_base_url);
        PublicWebUrl(url.trim_end_matches('/').to_string())
    }
}

#[derive(serde::Deserialize, Clone)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub database_name: String,
    pub require_ssl: bool,
}

#[derive(serde::Deserialize, Clone)]
pub struct FileStorageSettings {
    pub backend: String,
    pub local_root: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub max_file_size_bytes: u64,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub upload_token_ttl_minutes: u64,
}

#[derive(serde::Deserialize, Clone)]
pub struct FederationSettings {
    pub enabled: bool,
    pub public_base_url: String,
    pub require_https: bool,
    pub allow_insecure_private_network: bool,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub request_ttl_seconds: i64,
    pub allowed_remote_hosts: Vec<String>,
}

impl DatabaseSettings {
    pub fn connect_options(&self) -> PgConnectOptions {
        let ssl_mode = if self.require_ssl {
            PgSslMode::Require
        } else {
            PgSslMode::Prefer
        };
        PgConnectOptions::new()
            .host(&self.host)
            .username(&self.username)
            .password(self.password.expose_secret())
            .port(self.port)
            .ssl_mode(ssl_mode)
            .database(&self.database_name)
    }
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to determine the current directory");
    let configuration_directory = base_path.join("configuration");

    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.");
    let environment_filename = format!("{}.yaml", environment.as_str());
    let settings = config::Config::builder()
        .add_source(config::File::from(
            configuration_directory.join("base.yaml"),
        ))
        .add_source(config::File::from(
            configuration_directory.join(environment_filename),
        ))
        .add_source(
            config::Environment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;

    settings.try_deserialize::<Settings>()
}

pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{other} is not a supported environment. Use either `local` or `production`."
            )),
        }
    }
}
