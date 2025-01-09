#[macro_use] extern crate log;
mod runtime;
use actix_web::{web, HttpResponse};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	pretty_env_logger::formatted_builder()
		.parse_filters("off,actix_web=info,votebase=info")
		.parse_default_env()
		.init();

	actix_web::HttpServer::new(|| {
		actix_web::App::new()
			.wrap(actix_web::middleware::Logger::default())
			.service(execute_action)
			.service(execute_view)
	})
	.bind(("0.0.0.0", 8080))?
	.run()
	.await
}

#[derive(thiserror::Error, Debug)]
enum VotebaseError {
	#[error("internal error")]
	Internal,
}

impl From<runtime::DenoError> for VotebaseError {
	fn from(_value: runtime::DenoError) -> Self {
		VotebaseError::Internal
	}
}

impl VotebaseError {
	fn respond(&self, status_code: actix_web::http::StatusCode) -> HttpResponse {
		let res = HttpResponse::new(status_code);
		match self {
			Self::Internal => res.into(),
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
			Self::Internal => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
		}
	}

	fn error_response(&self) -> HttpResponse {
		self.respond(self.status_code())
	}
}

#[actix_web::post("/action/{path}")]
async fn execute_action(path: web::Path<String>, arg: web::Query<serde_json::Value>) -> Result<HttpResponse<()>, VotebaseError> {
	dbg!(path);
	let return_value = runtime::run_function(CONSTITUTION_CODE.to_string(), FUNCTION_NAME, arg.into_inner()).await?;
	dbg!(return_value);

	Ok(HttpResponse::with_body(actix_web::http::StatusCode::NO_CONTENT, ()))
}

#[actix_web::get("/view/{path}")]
async fn execute_view(path: web::Path<String>, query: web::Query<serde_json::Value>) -> Result<web::Json<serde_json::Value>, VotebaseError> {
	info!("hey");
	dbg!(path);
	let return_value = runtime::run_function(CONSTITUTION_CODE.to_string(), FUNCTION_NAME, query.into_inner()).await?;

	Ok(web::Json(dbg!(return_value)))
}


const CONSTITUTION_CODE: &'static str = r#"
console.log("wassup")

votebase.reg("hello", (arg: string) => {
	console.log(arg)
	return typeof arg === 'object'
})
"#;
const FUNCTION_NAME: &'static str = "hello";
