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
    pub redis_uri: Secret<String>,
}

#[derive(serde::Deserialize, Clone)]
pub struct ApplicationSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub base_url: String,
    pub hmac_secret: Secret<String>,
    pub cookie_secure: bool,
    pub enable_federation: bool,
    pub cors_allowed_origins: Vec<String>,
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
    pub local_node_id: Option<uuid::Uuid>,
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

    let mut settings: Settings = settings.try_deserialize::<Settings>()?;
    configure_federation_local_node_id(&mut settings);

    Ok(settings)
}

fn configure_federation_local_node_id(settings: &mut Settings) {
    if settings.federation.local_node_id.is_none() {
        let new_id = uuid::Uuid::new_v4();
        settings.federation.local_node_id = Some(new_id);

        // Persist to the environment-specific config file
        let base_path = std::env::current_dir().expect("Failed to determine the current directory");
        let configuration_directory = base_path.join("configuration");
        let config_path = configuration_directory.join("base.yaml");
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            let mut yaml_value = serde_yaml::from_str(&content)
                .unwrap_or(serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));

            if let Some(federation) = yaml_value.get_mut("federation") {
                if let Some(map) = federation.as_mapping_mut() {
                    map.insert(
                        serde_yaml::Value::String("local_node_id".to_string()),
                        serde_yaml::Value::String(new_id.to_string()),
                    );
                }
            } else {
                let mut fed_map = serde_yaml::Mapping::new();
                fed_map.insert(
                    serde_yaml::Value::String("local_node_id".to_string()),
                    serde_yaml::Value::String(new_id.to_string()),
                );
                if let Some(root_map) = yaml_value.as_mapping_mut() {
                    root_map.insert(
                        serde_yaml::Value::String("federation".to_string()),
                        serde_yaml::Value::Mapping(fed_map),
                    );
                }
            }

            if let Ok(new_content) = serde_yaml::to_string(&yaml_value) {
                let _ = std::fs::write(&config_path, new_content);
            }
        }
    }
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
