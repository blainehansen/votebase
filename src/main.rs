#[macro_use] extern crate log;
mod runtime;

mod error;
use error::VotebaseError;
use actix_web::{web, HttpResponse};

type PgPool = sqlx::Pool<sqlx::Postgres>;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	pretty_env_logger::formatted_builder()
		.parse_filters("off,actix_web=info,votebase=info")
		.parse_default_env()
		.init();

	let max_connections = std::env::var("DATABASE_MAX_CONNECTIONS").ok()
		.and_then(|s| s.parse().ok()).unwrap_or(5);

	#[cfg(debug_assertions)]
	let admin_user = std::env::var("VOTEBASE_USER").unwrap_or("dev_user".to_string());
	#[cfg(not(debug_assertions))]
	let admin_user = std::env::var("VOTEBASE_USER").expect("VOTEBASE_USER must be set");

	#[cfg(debug_assertions)]
	let admin_pass = std::env::var("VOTEBASE_PASS").unwrap_or("dev_password".to_string());
	#[cfg(not(debug_assertions))]
	let admin_pass = std::env::var("VOTEBASE_PASS").expect("VOTEBASE_PASS must be set");
	let database_url = pg_connect_options(&admin_user, &admin_pass);
	let pool: PgPool = sqlx::postgres::PgPoolOptions::new()
		// TODO am I sure about even setting this at all?
		.max_connections(max_connections)
		.connect_with(database_url.options).await.unwrap();

	actix_web::HttpServer::new(move || {
		actix_web::App::new()
			.app_data(web::Data::new(pool.clone()))
			.wrap(actix_web::middleware::Logger::default())
			.service(execute_action)
			.service(execute_view)
	})
	.bind(("0.0.0.0", 8080))?
	.run()
	.await
}

static GLOBAL_PG_OPTIONS: once_cell::sync::Lazy<PgOpt> = once_cell::sync::Lazy::new(|| {
	#[cfg(debug_assertions)]
	let port = std::env::var("DB_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(5432);
	#[cfg(not(debug_assertions))]
	let port = std::env::var("DB_PORT").expect("DB_PORT must be set").parse().expect("DB_PORT must be a valid port number");

	#[cfg(debug_assertions)]
	let host = std::env::var("DB_HOST").unwrap_or("localhost".to_string());
	#[cfg(not(debug_assertions))]
	let host = std::env::var("DB_HOST").expect("DB_HOST must be set");

	#[cfg(debug_assertions)]
	let database = std::env::var("DB_DATABASE").unwrap_or("dev_db".to_string());
	#[cfg(not(debug_assertions))]
	let database = std::env::var("DB_DATABASE").expect("DB_DATABASE must be set");

	let options = sqlx::postgres::PgConnectOptions::new_without_pgpass()
		.port(port)
		.host(&host)
		.database(&database);
	PgOpt { options, database, password: "".to_string() }
});

#[derive(Debug, Clone)]
struct PgOpt {
	options: sqlx::postgres::PgConnectOptions,
	database: String,
	password: String,
}
impl PgOpt {
	fn database(self, d: &str) -> Self {
		PgOpt { options: self.options.database(d), database: d.to_string(), ..self }
	}
	fn password(self, p: &str) -> Self {
		PgOpt { options: self.options.password(p), password: p.to_string(), ..self }
	}
	fn username(self, u: &str) -> Self {
		PgOpt { options: self.options.username(u), ..self }
	}
}

fn pg_connect_options(username: &str, password: &str) -> PgOpt {
	let password = password.to_string();
	GLOBAL_PG_OPTIONS.clone().username(username).password(&password)
}
fn map_sqlx_not_found(error: sqlx::Error, fn_path: &FnPath) -> VotebaseError {
	match error {
		sqlx::Error::RowNotFound => VotebaseError::FnNotFoundError(fn_path.clone()),
		e => e.into(),
	}
}

#[derive(Debug, Clone)]
struct FnPath {
	ruleset_full_path: String,
	fn_name: String,
}

const PATH_DELIMITER: char = '|';

impl actix_web::FromRequest for FnPath {
	type Error = actix_web::Error;
	type Future = std::future::Ready<Result<Self, Self::Error>>;

	fn from_request(req: &actix_web::HttpRequest, _: &mut actix_web::dev::Payload) -> Self::Future {
		let path = match req.match_info().get("path") {
			Some(p) => p,
			None => return std::future::ready(Err(actix_web::error::ErrorBadRequest("path is required"))),
		};
		let result = match path.rsplit_once(PATH_DELIMITER) {
			Some((r, f)) => Ok(FnPath{ ruleset_full_path: r.to_string(), fn_name: f.to_string() }),
			None => Err(actix_web::error::ErrorBadRequest("all paths must have at least one ruleset name")),
		};

		std::future::ready(result)
	}
}

fn construct_role(fn_path: &FnPath, fn_type: runtime::FnType) -> String {
	format!("{}|{}|{}",
		match fn_type {
			runtime::FnType::Action => "action",
			runtime::FnType::View => "view"
		},
		fn_path.ruleset_full_path, fn_path.fn_name,
	)
}

#[actix_web::post("/action/{path}")]
async fn execute_action(
	fn_path: FnPath,
	arg: web::Query<serde_json::Value>,
	pool: web::Data<PgPool>,
) -> Result<HttpResponse<()>, VotebaseError> {
	let pool = pool.get_ref();

	let action = sqlx::query!("
		select code, action_pass as pass
		from votebase_catalog.ruleset
		where full_path = $1 and $2 = ANY(actions)
	", &fn_path.ruleset_full_path, &fn_path.fn_name)
		.fetch_one(pool).await.map_err(|e| map_sqlx_not_found(e, &fn_path))?;

	let fn_type = runtime::FnType::Action;
	let action_role = construct_role(&fn_path, fn_type);
	let action_role_url = pg_connect_options(&action_role, &action.pass);

	let new_ruleset_id = runtime::run_function::<Option<String>>(
		fn_path.ruleset_full_path, action.code, &fn_path.fn_name, arg.into_inner(), fn_type,
		action_role_url, GLOBAL_PG_OPTIONS.clone(), pool.clone(),
	).await?;

	if let Some(new_ruleset_id) = new_ruleset_id {
		let new_ruleset_id = new_ruleset_id.parse::<sqlx::types::Uuid>()?;
		info!("apply_candidate {new_ruleset_id}");
		runtime::replace_ruleset(pool, new_ruleset_id).await?;
	}

	Ok(HttpResponse::with_body(actix_web::http::StatusCode::NO_CONTENT, ()))
}

#[actix_web::get("/view/{path}")]
async fn execute_view(
	fn_path: FnPath,
	query: web::Query<serde_json::Value>,
	pool: web::Data<PgPool>,
	// TODO use Either here to allow json or html?
) -> Result<web::Html, VotebaseError> {
	let pool = pool.get_ref();

	let view = sqlx::query!("
		select code, view_pass as pass
		from votebase_catalog.ruleset
		where full_path = $1 and $2 = ANY(views)
	", &fn_path.ruleset_full_path, &fn_path.fn_name)
		.fetch_one(pool).await.map_err(|e| map_sqlx_not_found(e, &fn_path))?;

	let fn_type = runtime::FnType::View;
	let view_role = construct_role(&fn_path, fn_type);
	let view_role_url = pg_connect_options(&view_role, &view.pass);
	let return_value: String = runtime::run_function(
		fn_path.ruleset_full_path, view.code, &fn_path.fn_name, query.into_inner(), fn_type,
		view_role_url, GLOBAL_PG_OPTIONS.clone(), pool.clone(),
	).await?;
	Ok(web::Html::new(return_value))
}
