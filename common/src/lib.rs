pub mod runtime;

pub type PgPool = sqlx::Pool<sqlx::Postgres>;
pub type PgOpt = sqlx::postgres::PgConnectOptions;
