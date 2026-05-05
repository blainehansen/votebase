use crate::{runtime, FnPath};
use actix_web::HttpResponse;

#[derive(thiserror::Error, Debug)]
pub enum VotebaseError {
	#[error("function {}|{} not found", .0.ruleset_full_path, .0.fn_name)]
	FnNotFoundError(FnPath),
	#[error("ruleset {} not found", .0)]
	RulesetNotFoundError(String),

	#[error(transparent)]
	RuntimeError(#[from] runtime::RuntimeError),
	#[error(transparent)]
	RunActionError(#[from] votebase_common::runtime::RunActionError),
	#[error(transparent)]
	PostgresError(#[from] votebase_common::postgres::Error),
	#[error(transparent)]
	PoolError(#[from] votebase_common::deadpool::PoolError),
	#[error(transparent)]
	UuidParseError(#[from] uuid::Error)
}


impl VotebaseError {
	fn respond(&self, status_code: actix_web::http::StatusCode) -> HttpResponse {
		log::error!("{}", self);
		let res = HttpResponse::new(status_code);
		res.into()

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
			Self::FnNotFoundError(_) | Self::RulesetNotFoundError(_) => actix_web::http::StatusCode::NOT_FOUND,
			| Self::RuntimeError(_)
			| Self::PostgresError(_)
			| Self::PoolError(_)
			| Self::UuidParseError(_)
				=> actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
		}
	}

	fn error_response(&self) -> HttpResponse {
		self.respond(self.status_code())
	}
}
