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

	let database_url = std::env::var("DATABASE_URL")
		.unwrap_or_else(|_| "postgres://dev_user:dev_password@localhost/dev_db".to_string());

	let pool: PgPool = sqlx::postgres::PgPoolOptions::new()
		.max_connections(max_connections)
		.connect(&database_url).await.unwrap();

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

#[derive(Debug)]
struct FnPath {
	ruleset: String,
	function: String,
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
			Some((r, f)) => Ok(FnPath{ ruleset: r.to_string(), function: f.to_string() }),
			None => Err(actix_web::error::ErrorBadRequest("all paths must have at least one ruleset")),
		};

		std::future::ready(result)
	}
}


#[actix_web::post("/action/{path}")]
async fn execute_action(
	path: FnPath,
	arg: web::Query<serde_json::Value>,
	pool: web::Data<PgPool>,
) -> Result<HttpResponse<()>, VotebaseError> {
	// TODO do a join or something to get function_name?
	let (ruleset_code,): (String,) = sqlx::query_as("select code from rulesets where ruleset_path = $1")
		.bind(path.ruleset)
		.fetch_one(pool.get_ref()).await?;

	// TODO have to get this from the database
	let action_role_url = "TODO";
	let return_value = runtime::run_function(
		ruleset_code, &path.function, arg.into_inner(), runtime::FnType::Action, action_role_url,
	).await?;
	dbg!(return_value);
	// TODO use the return value, perhaps validating first to a known structure you can use to modify the database

	Ok(HttpResponse::with_body(actix_web::http::StatusCode::NO_CONTENT, ()))
}

#[actix_web::get("/view/{path}")]
async fn execute_view(
	path: FnPath,
	query: web::Query<serde_json::Value>,
	pool: web::Data<PgPool>,
) -> Result<web::Json<serde_json::Value>, VotebaseError> {
	// TODO do a join or something to get function_name?
	let (ruleset_code,): (String,) = sqlx::query_as("select code from rulesets where ruleset_path = $1")
		.bind(path.ruleset)
		.fetch_one(pool.get_ref()).await?;

	// TODO have to get this from the database
	let view_role_url = "TODO";
	let return_value = runtime::run_function(
		ruleset_code, FUNCTION_NAME, query.into_inner(), runtime::FnType::View, view_role_url,
	).await?;
	Ok(web::Json(return_value))
}

// we need a route to insert a candidate constitution, because we need the ability to check that the constitution is right
// split_first/last

#[derive(Debug)]
struct Ruleset {
	final_schema: String,
	migration_sql: String,
	text: String,
}


const FUNCTION_NAME: &'static str = "hello";
