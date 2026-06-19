use sqlx::postgres::PgPoolOptions;

const ROOT_PASSWORD_HASH: &str = "$argon2id$v=19$m=15000,t=2,p=1$/JY8nmHDxi8UigW19vfwLQ$qy3rphJ4BvbYHLhfMzuC6FG2OghQTI4KpXVuQV4vmY0";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let database_url = std::env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new().connect(&database_url).await?;
    let result = sqlx::query("UPDATE users SET password_hash = $1 WHERE username = 'root'")
        .bind(ROOT_PASSWORD_HASH)
        .execute(&pool)
        .await?;
    println!("updated rows: {}", result.rows_affected());
    Ok(())
}
