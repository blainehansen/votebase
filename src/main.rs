#[macro_use] extern crate log;
mod runtime;
use actix_web::{web, HttpResponse};

type PgPool = sqlx::Pool<sqlx::Postgres>;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	pretty_env_logger::formatted_builder()
		.parse_filters("off,actix_web=info,votebase=info")
		.parse_default_env()
		.init();

	let pool: PgPool = sqlx::postgres::PgPoolOptions::new()
		.max_connections(5)
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

#[actix_web::post("/action/{path}")]
async fn execute_action(
	// TODO make your own extractor to get the full safe path
	path: web::Path<String>,
	arg: web::Query<serde_json::Value>,
	pool: web::Data<PgPool>,
) -> Result<HttpResponse<()>, VotebaseError> {
	// TODO do a join or something to get function_name?
	let (constitution_code,): (String,) = sqlx::query_as("select constitution_code from constitutions where path = $1")
		.bind(path.into_inner())
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



// const CONSTITUTION_CODE: &'static str = r#"
// console.log("wassup")

// votebase.reg("hello", (arg: string) => {
// 	console.log(arg)
// 	return typeof arg === 'object'
// })
// "#;
const FUNCTION_NAME: &'static str = "hello";


#[derive(thiserror::Error, Debug)]
enum VotebaseError {
	#[error("internal error")]
	DenoError(#[from] runtime::DenoError),
	#[error("internal error")]
	SqlxError(#[from] sqlx::Error)
}

impl VotebaseError {
	fn respond(&self, status_code: actix_web::http::StatusCode) -> HttpResponse {
		error!("{}", self);
		let res = HttpResponse::new(status_code);
		match self {
			Self::DenoError(_) | Self::SqlxError(_) => res.into(),
		}

		// let mut buf = web::BytesMut::new();
		// let _ = std::write!(helpers::MutWriter(&mut buf), "{}", self);

		// let mime = mime::TEXT_PLAIN_UTF_8.try_into_value().unwrap();
		// res.headers_mut().insert(actix_web::http::header::CONTENT_TYPE, mime);

		// res.set_body(actix_web::body::BoxBody::new(buf))
	}
}

impl actix_web::ResponseError for VotebaseError {
	fn status_code(&self) -> actix_web::http::StatusCode {
		match self {
			Self::DenoError(_) | Self::SqlxError(_) => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
		}
	}

	fn error_response(&self) -> HttpResponse {
		self.respond(self.status_code())
	}
}
