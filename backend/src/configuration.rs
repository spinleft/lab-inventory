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
    #[serde(deserialize_with = "deserialize_list")]
    pub cors_allowed_origins: Vec<String>,
    /// Password to give `root` on the next start, while it still carries the
    /// one seeded by the migrations.
    ///
    /// Deployments keep this in their environment file, so it is read on every
    /// start; it is only ever applied to the seeded password, never to one an
    /// operator has since chosen.
    #[serde(default)]
    pub initial_root_password: Option<Secret<String>>,
    /// Whether to refuse to start while `root` still carries the seeded
    /// password.
    ///
    /// On in production, where that password would be the one published in
    /// this repository.
    #[serde(default)]
    pub require_root_password_rotation: bool,
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
    /// Whether the server applies pending migrations on start.
    ///
    /// Off for local development, where `scripts/init_db` owns the schema and
    /// an implicit migration would hide a forgotten one. On in production, so
    /// that upgrading a deployment is just pulling a newer image.
    #[serde(default)]
    pub run_migrations: bool,
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
    #[serde(deserialize_with = "deserialize_list")]
    pub allowed_remote_hosts: Vec<String>,
}

/// Reads a list that arrives either as a real sequence or as one
/// comma-separated string.
///
/// Environment variables can only carry strings, and these lists are among the
/// settings a deployment most often has to override — so requiring a mounted
/// YAML file just to add one CORS origin would be the wrong trade. Blank
/// entries are dropped, because an unset variable in a compose file reaches
/// here as an empty string rather than as an absent key.
fn deserialize_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum SequenceOrString {
        Sequence(Vec<String>),
        String(String),
    }

    use serde::Deserialize;

    let entries = match SequenceOrString::deserialize(deserializer)? {
        SequenceOrString::Sequence(entries) => entries,
        SequenceOrString::String(value) => value.split(',').map(str::to_string).collect(),
    };
    Ok(entries
        .into_iter()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Wrapper {
        #[serde(deserialize_with = "deserialize_list")]
        entries: Vec<String>,
    }

    fn parse(json: &str) -> Vec<String> {
        serde_json::from_str::<Wrapper>(json).unwrap().entries
    }

    #[test]
    fn a_yaml_sequence_is_read_as_a_list() {
        assert_eq!(
            parse(r#"{ "entries": ["https://a.example.com", "https://b.example.com"] }"#),
            vec!["https://a.example.com", "https://b.example.com"]
        );
    }

    /// How the setting arrives when it comes from an environment variable.
    #[test]
    fn a_comma_separated_string_is_split() {
        assert_eq!(
            parse(r#"{ "entries": "https://a.example.com,https://b.example.com" }"#),
            vec!["https://a.example.com", "https://b.example.com"]
        );
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        assert_eq!(
            parse(r#"{ "entries": "https://a.example.com , https://b.example.com" }"#),
            vec!["https://a.example.com", "https://b.example.com"]
        );
    }

    /// An unset variable in a compose file reaches the server as an empty
    /// string, which must mean "no origins" rather than "one blank origin".
    #[test]
    fn an_empty_string_is_an_empty_list() {
        assert!(parse(r#"{ "entries": "" }"#).is_empty());
    }

    #[test]
    fn blank_entries_are_dropped() {
        assert_eq!(
            parse(r#"{ "entries": "https://a.example.com,,  ," }"#),
            vec!["https://a.example.com"]
        );
    }
}
