use crate::domain::GuestRegistrationCode;
use anyhow::Context;
use hmac::{Hmac, Mac};
use redis::aio::ConnectionManager;
use secrecy::{ExposeSecret, Secret};
use sha2::Sha256;
use std::net::IpAddr;

const RATE_LIMIT_WINDOW_SECONDS: usize = 10 * 60;
const RATE_LIMIT_MAX_REQUESTS: i64 = 10;
const REGISTRATION_CODE_DOMAIN: &[u8] = b"lab-inventory:guest-registration-code:v1\0";
const RATE_LIMIT_DOMAIN: &[u8] = b"lab-inventory:guest-registration-rate-limit:v1\0";

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct GuestRegistrationHasher {
    secret: Secret<String>,
}

impl GuestRegistrationHasher {
    pub fn new(secret: Secret<String>) -> Self {
        Self { secret }
    }

    pub fn hash_code(&self, code: &GuestRegistrationCode) -> String {
        self.digest(
            REGISTRATION_CODE_DOMAIN,
            code.as_ref().expose_secret().as_bytes(),
        )
    }

    fn rate_limit_subject(&self, ip: IpAddr) -> String {
        self.digest(RATE_LIMIT_DOMAIN, ip.to_string().as_bytes())
    }

    fn digest(&self, domain: &[u8], value: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(self.secret.expose_secret().as_bytes())
            .expect("HMAC accepts keys of any size");
        mac.update(domain);
        mac.update(value);
        mac.finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

pub struct GuestRegistrationRateLimit {
    pub allowed: bool,
    pub retry_after_seconds: u64,
}

#[derive(Clone)]
pub struct GuestRegistrationRateLimiter {
    connection: ConnectionManager,
    namespace: String,
    hasher: GuestRegistrationHasher,
}

impl GuestRegistrationRateLimiter {
    pub async fn new(
        redis_uri: &str,
        database_name: &str,
        hasher: GuestRegistrationHasher,
    ) -> Result<Self, anyhow::Error> {
        let client = redis::Client::open(redis_uri).context("Invalid Redis URI")?;
        let connection = ConnectionManager::new(client)
            .await
            .context("Failed to connect the guest registration rate limiter to Redis")?;
        let namespace = hasher.digest(RATE_LIMIT_DOMAIN, database_name.as_bytes());
        Ok(Self {
            connection,
            namespace,
            hasher,
        })
    }

    pub async fn check(&self, ip: IpAddr) -> Result<GuestRegistrationRateLimit, anyhow::Error> {
        let subject = self.hasher.rate_limit_subject(ip);
        let key = format!("guest-registration:{}:{subject}", self.namespace);
        let script = redis::Script::new(
            r#"
            local count = redis.call('INCR', KEYS[1])
            if count == 1 then
                redis.call('EXPIRE', KEYS[1], ARGV[1])
            end
            local ttl = redis.call('TTL', KEYS[1])
            return { count, ttl }
            "#,
        );
        let mut connection = self.connection.clone();
        let (count, ttl): (i64, i64) = script
            .key(key)
            .arg(RATE_LIMIT_WINDOW_SECONDS)
            .invoke_async(&mut connection)
            .await
            .context("Failed to update guest registration rate limit")?;

        Ok(GuestRegistrationRateLimit {
            allowed: count <= RATE_LIMIT_MAX_REQUESTS,
            retry_after_seconds: ttl.max(1) as u64,
        })
    }
}
