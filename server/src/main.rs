#[macro_use] extern crate log;
use votebase_common::{runtime, PgPool};

mod error;
use error::VotebaseError;
use actix_web::{web, HttpResponse};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	pretty_env_logger::formatted_builder()
		.parse_filters("off,actix_web=info,votebase=info")
		.parse_default_env()
		.init();

	#[cfg(debug_assertions)]
	let db_port = std::env::var("DB_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(5432);
	#[cfg(not(debug_assertions))]
	let db_port = std::env::var("DB_PORT").expect("DB_PORT must be set").parse().expect("DB_PORT must be a valid port number");

	#[cfg(debug_assertions)]
	let db_host = std::env::var("DB_HOST").unwrap_or("localhost".to_string());
	#[cfg(not(debug_assertions))]
	let db_host = std::env::var("DB_HOST").expect("DB_HOST must be set");

	#[cfg(debug_assertions)]
	let db_database = std::env::var("DB_DATABASE").unwrap_or("dev_db".to_string());
	#[cfg(not(debug_assertions))]
	let db_database = std::env::var("DB_DATABASE").expect("DB_DATABASE must be set");

	let db_votebase_user = "votebase_server";

	#[cfg(debug_assertions)]
	let db_votebase_pass = std::env::var("VOTEBASE_PASS").unwrap_or("votebase_server_dev_pass".to_string());
	#[cfg(not(debug_assertions))]
	let db_votebase_pass = std::env::var("VOTEBASE_PASS").expect("VOTEBASE_PASS must be set");

	let database_url = sqlx::postgres::PgConnectOptions::new_without_pgpass()
		.port(db_port)
		.host(&db_host)
		.database(&db_database)
		.username(db_votebase_user)
		.password(&db_votebase_pass);

	let max_connections = std::env::var("DATABASE_MAX_CONNECTIONS").ok().and_then(|s| s.parse().ok()).unwrap_or(5);

	let pool: PgPool = sqlx::postgres::PgPoolOptions::new()
		// TODO am I sure about even setting this at all?
		.max_connections(max_connections)
		.connect_with(database_url).await.unwrap();

	let queue_pool = pool.clone();
	tokio::task::spawn(async move {
		let result = sqlx::query!(r#"
			select id, scheduled_time
			from votebase_catalog.detached_scheduled_action;
		"#).fetch_all(&queue_pool).await;

		match result {
			Err(e) => { error!("failed to fetch detached_scheduled_actions: {}", e) },
			Ok(detached_scheduled_actions) => {
				info!("queuing {} detached_scheduled_actions", detached_scheduled_actions.len());
				let server_pg_opt = queue_pool.connect_options().as_ref().clone();
				for scheduled_action in detached_scheduled_actions {
					runtime::queue_scheduled_action(
						queue_pool.clone(), server_pg_opt.clone(),
						scheduled_action.id, scheduled_action.scheduled_time,
					);
				}
			},
		}
	});

	#[cfg(debug_assertions)]
	let host = std::env::var("VOTEBASE_HOST").unwrap_or("0.0.0.0".to_string());
	#[cfg(not(debug_assertions))]
	let host = std::env::var("VOTEBASE_HOST").expect("VOTEBASE_HOST must be set");

	#[cfg(debug_assertions)]
	let port = std::env::var("VOTEBASE_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
	#[cfg(not(debug_assertions))]
	let port = std::env::var("VOTEBASE_PORT").ok().and_then(|p| p.parse().ok()).expect("VOTEBASE_PORT must be set");

	actix_web::HttpServer::new(move || {
		let app = actix_web::App::new()
			.app_data(web::Data::new(pool.clone()))
			.wrap(actix_web::middleware::Logger::default())
			.service(get_rulesets)
			.service(get_ruleset_views)
			.service(get_ruleset_detail)
			.service(execute_action)
			.service(execute_view);

		#[cfg(debug_assertions)]
		let app = app.service(debug_advance_time).wrap(actix_cors::Cors::permissive());

		app
	})
	.bind((host, port))?
	.run()
	.await
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

#[derive(Debug, serde::Serialize)]
struct RulesetListing {
	full_path: String,
}

#[actix_web::get("/rulesets")]
async fn get_rulesets(
	pool: web::Data<PgPool>,
) -> Result<web::Json<Vec<RulesetListing>>, VotebaseError> {
	let pool = pool.get_ref();

	let rulesets = sqlx::query_as!(RulesetListing, "
		select full_path
		from votebase_catalog.ruleset
	").fetch_all(pool).await?;

	Ok(web::Json(rulesets))
}

#[actix_web::get("/ruleset-views/{ruleset_full_path}")]
async fn get_ruleset_views(
	ruleset_full_path: web::Path<String>,
	pool: web::Data<PgPool>,
) -> Result<web::Json<Vec<String>>, VotebaseError> {
	let ruleset_full_path = ruleset_full_path.into_inner();
	let pool = pool.get_ref();

	let r = sqlx::query!(r#"
		select views
		from votebase_catalog.ruleset
		where full_path = $1
	"#, &ruleset_full_path).fetch_one(pool).await.map_err(|e| match e {
		sqlx::Error::RowNotFound => VotebaseError::RulesetNotFoundError(ruleset_full_path),
		e => e.into(),
	})?;

	Ok(web::Json(r.views))
}

#[derive(Debug, serde::Serialize)]
struct RulesetDetail {
	views: Vec<String>,
	code: String,
	db_schema: String,
}

#[actix_web::get("/ruleset-detail/{ruleset_full_path}")]
async fn get_ruleset_detail(
	ruleset_full_path: web::Path<String>,
	pool: web::Data<PgPool>,
) -> Result<web::Json<RulesetDetail>, VotebaseError> {
	let ruleset_full_path = ruleset_full_path.into_inner();
	let pool = pool.get_ref();

	let ruleset = sqlx::query_as!(RulesetDetail, r#"
		select views, code, db_schema
		from votebase_catalog.ruleset
		where full_path = $1
	"#, &ruleset_full_path).fetch_one(pool).await.map_err(|e| match e {
		sqlx::Error::RowNotFound => VotebaseError::RulesetNotFoundError(ruleset_full_path),
		e => e.into(),
	})?;

	Ok(web::Json(ruleset))
}

#[actix_web::post("/fn/action/{path}")]
async fn execute_action(
	fn_path: FnPath,
	arg: web::Json<serde_json::Value>,
	pool: web::Data<PgPool>,
) -> Result<HttpResponse<()>, VotebaseError> {
	let pool = pool.get_ref();
	let server_pg_opt = pool.connect_options().as_ref().clone();

	let action = sqlx::query!("
		select code, action_pass, migrator_pass
		from votebase_catalog.ruleset
		where full_path = $1 and $2 = ANY(actions)
	", &fn_path.ruleset_full_path, &fn_path.fn_name)
		.fetch_one(pool).await.map_err(|e| map_sqlx_not_found(e, &fn_path))?;

	runtime::run_action(
		fn_path.ruleset_full_path, action.code, &fn_path.fn_name, &action.action_pass, &action.migrator_pass, arg.into_inner(),
		server_pg_opt, pool,
	).await?;

	Ok(HttpResponse::with_body(actix_web::http::StatusCode::NO_CONTENT, ()))
}

#[actix_web::get("/fn/view/{path}")]
async fn execute_view(
	fn_path: FnPath,
	query: web::Query<serde_json::Value>,
	pool: web::Data<PgPool>,
	// TODO use Either here to allow json or html?
) -> Result<web::Html, VotebaseError> {
	let pool = pool.get_ref();
	let server_pg_opt = pool.connect_options().as_ref().clone();

	let view = sqlx::query!("
		select code, view_pass as pass
		from votebase_catalog.ruleset
		where full_path = $1 and $2 = ANY(views)
	", &fn_path.ruleset_full_path, &fn_path.fn_name)
		.fetch_one(pool).await.map_err(|e| map_sqlx_not_found(e, &fn_path))?;

	let return_value = runtime::run_view(
		fn_path.ruleset_full_path, view.code, &fn_path.fn_name, &view.pass, query.into_inner(),
		server_pg_opt, pool,
	).await?;
	Ok(web::Html::new(return_value))
}

#[cfg(debug_assertions)]
#[actix_web::post("/__debug_advance_time")]
async fn debug_advance_time(_amount: web::Json<()>) -> HttpResponse<()> {

	HttpResponse::with_body(actix_web::http::StatusCode::NO_CONTENT, ())
}
