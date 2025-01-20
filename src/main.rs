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

	let pool: PgPool = sqlx::postgres::PgPoolOptions::new()
		// TODO make this configurable
		.max_connections(5)
		// TODO make this configurable
		.connect("postgres://dev_user:dev_password@localhost/dev_db").await.unwrap();

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
	let (constitution_code,): (String,) = sqlx::query_as("select constitution_code from constitutions where rule")
		.bind(path.ruleset)
		.fetch_one(pool.get_ref()).await?;

	let return_value = runtime::run_function(constitution_code, FUNCTION_NAME, arg.into_inner()).await?;
	dbg!(return_value);
	// TODO use the return value, perhaps validating first to a known structure you can use to modify the database

	Ok(HttpResponse::with_body(actix_web::http::StatusCode::NO_CONTENT, ()))
}

#[actix_web::get("/view/{path}")]
async fn execute_view(
	path: web::Path<String>,
	query: web::Query<serde_json::Value>,
	pool: web::Data<PgPool>,
) -> Result<web::Json<serde_json::Value>, VotebaseError> {
	// TODO do a join or something to get function_name?
	let (constitution_code,): (String,) = sqlx::query_as("select constitution_code from constitutions where path = $1")
		.bind(path.into_inner())
		.fetch_one(pool.get_ref()).await?;

	let return_value = runtime::run_function(constitution_code, FUNCTION_NAME, query.into_inner()).await?;

	Ok(web::Json(dbg!(return_value)))
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
