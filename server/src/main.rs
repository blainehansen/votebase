use votebase_common::{runtime, PgPool, PgClient, PgConfig, postgres, deadpool, queries};

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

	let mut config = postgres::Config::new();
	config
		.port(db_port)
		.host(&db_host)
		.dbname(&db_database)
		.user(db_votebase_user)
		.password(&db_votebase_pass);

	let max_connections = std::env::var("DATABASE_MAX_CONNECTIONS").ok().and_then(|s| s.parse().ok()).unwrap_or(5);

	let pool = deadpool::Pool::builder(deadpool::Manager::new(config.clone(), postgres::NoTls))
		.max_size(max_connections as usize)
		.build().expect("Failed to create pool.");

	// Spawn background task queueing
	let queue_pool = pool.clone();
	let server_pg_config = config.clone(); // Use config for queueing
	tokio::task::spawn(async move {
		let mut client = match queue_pool.get().await {
			Ok(client) => client,
			Err(e) => {
				log::error!("Failed to get client for queueing background tasks: {}", e);
				return;
			}
		};

		// Queue detached scheduled actions
		match queries::server::get_all_detached_scheduled_actions().bind(&client).all().await {
			Err(e) => log::error!("failed to fetch detached_scheduled_actions: {}", e),
			Ok(detached_scheduled_actions) => {
				log::info!("queuing {} detached_scheduled_actions", detached_scheduled_actions.len());
				for action in detached_scheduled_actions {
					// queue_scheduled_action needs update to accept config
					runtime::queue_scheduled_action(
						queue_pool.clone(), server_pg_config.clone(), votebase_common::ScheduledActionKind::DetachedScheduled, // Correct kind
						action.id, action.scheduled_time,
					);
				}
			},
		}

		// Queue detached recurring actions
		match queries::server::get_all_pending_detached_recurring_actions().bind(&client).all().await {
			Err(e) => log::error!("failed to fetch detached_recurring_actions: {}", e),
			Ok(detached_recurring_actions) => {
				log::info!("queuing {} detached_recurring_actions", detached_recurring_actions.len());
				for action in detached_recurring_actions {
					// queue_scheduled_action needs update to accept config
					runtime::queue_scheduled_action(
						queue_pool.clone(), server_pg_config.clone(), votebase_common::ScheduledActionKind::DetachedRecurring,
						action.id, action.next_scheduled_time.and_utc(),
					);
				}
			},
		}

		// insert into votebase_catalog.detached_recurring_action
		// (description, "start", recurrence_granularity, recurrence_multiplier, full_path, action_name, action_arg)
		// values ('', current_timestamp, 'Day', 1, 'root', 'my_action', 'null'::json);
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

// TODO: Update this function or remove if not needed. Clorinde returns Option for `opt()` and `one()`.
// fn map_sqlx_not_found(error: sqlx::Error, fn_path: &FnPath) -> VotebaseError {
// 	match error {
// 		sqlx::Error::RowNotFound => VotebaseError::FnNotFoundError(fn_path.clone()),
// 		e => e.into(),
// 	}
// }

// Helper to map postgres errors, specifically for not found cases if needed
fn map_pg_error(error: postgres::Error, fn_path: Option<&FnPath>, ruleset_path: Option<&str>) -> VotebaseError {
	// TODO: Check if postgres::Error has a specific variant for row not found or similar
	// For now, assume specific queries handle Option return for not found cases.
	// If a query expects a row and doesn't get one, `one()` will return an error.
	// We might need more context to distinguish "not found" from other errors.
	if let Some(db_err) = error.as_db_error() {
		log::error!("Database error: {:?}", db_err);
		// Potentially check db_err.code() here if needed
	} else {
		log::error!("Postgres error: {}", error);
	}

	// Tentative mapping: If we have a path, assume error might be related to not finding that resource
	if let Some(fp) = fn_path {
		VotebaseError::FnNotFoundError(fp.clone()) // Or just return PostgresError(error)?
	} else if let Some(rp) = ruleset_path {
		VotebaseError::RulesetNotFoundError(rp.to_string()) // Or just return PostgresError(error)?
	} else {
		VotebaseError::PostgresError(error)
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
) -> Result<web::Json<Vec<queries::server::GetRulesets>>, VotebaseError> {
	let client = pool.get().await?; // Get client

	let rulesets = queries::server::get_rulesets()
		.bind(&client)
		.all().await?;

	Ok(web::Json(rulesets))
}

#[actix_web::get("/ruleset-views/{ruleset_full_path}")]
async fn get_ruleset_views(
	ruleset_full_path_param: web::Path<String>, // Renamed to avoid conflict
	pool: web::Data<PgPool>,
) -> Result<web::Json<Vec<String>>, VotebaseError> {
	let ruleset_full_path = ruleset_full_path_param.into_inner();
	let client = pool.get().await?; // Get client

	let ruleset = queries::server::get_ruleset_views()
		.bind(&client, &ruleset_full_path)
		.opt().await? // Use opt() for optional result
		.ok_or_else(|| VotebaseError::RulesetNotFoundError(ruleset_full_path))?;

	Ok(web::Json(ruleset.views.unwrap_or_default())) // Handle potential null from DB
}

#[derive(Debug, serde::Serialize)]
struct RulesetDetail {
	views: Vec<String>,
	code: String,
	db_schema: String,
}

#[actix_web::get("/ruleset-detail/{ruleset_full_path}")]
async fn get_ruleset_detail(
	ruleset_full_path_param: web::Path<String>, // Renamed
	pool: web::Data<PgPool>,
) -> Result<web::Json<queries::server::GetRulesetDetail>, VotebaseError> { // Use generated struct
	let ruleset_full_path = ruleset_full_path_param.into_inner();
	let client = pool.get().await?; // Get client

	let ruleset = queries::server::get_ruleset_detail()
		.bind(&client, &ruleset_full_path)
		.opt().await? // Use opt() for optional result
		.ok_or_else(|| VotebaseError::RulesetNotFoundError(ruleset_full_path))?;

	Ok(web::Json(ruleset))
}

#[actix_web::post("/fn/action/{path}")]
async fn execute_action(
	fn_path: FnPath,
	arg: web::Json<serde_json::Value>,
	pool: web::Data<PgPool>,
) -> Result<HttpResponse<()>, VotebaseError> {
	let client = pool.get().await?; // Get client
	let server_pg_config = pool.config().clone(); // Get config from pool

	let action_details = queries::server::get_action_details()
		.bind(&client, &fn_path.ruleset_full_path, &fn_path.fn_name)
		.opt().await? // Use opt()
		.ok_or_else(|| VotebaseError::FnNotFoundError(fn_path.clone()))?;

	// run_action needs update to accept config and pool/client appropriately
	runtime::run_action(
		fn_path.ruleset_full_path, action_details.code, &fn_path.fn_name,
		&action_details.action_pass, &action_details.migrator_pass, arg.into_inner(),
		server_pg_config, &pool, // Pass config and pool (needs refactor in run_action)
	).await?;

	Ok(HttpResponse::with_body(actix_web::http::StatusCode::NO_CONTENT, ()))
}

#[actix_web::get("/fn/view/{path}")]
async fn execute_view(
	fn_path: FnPath,
	query: web::Query<serde_json::Value>,
	pool: web::Data<PgPool>,
) -> Result<web::Html<String>, VotebaseError> { // Explicit type for Html
	let client = pool.get().await?; // Get client
	let server_pg_config = pool.config().clone(); // Get config from pool

	let view_details = queries::server::get_view_details()
		.bind(&client, &fn_path.ruleset_full_path, &fn_path.fn_name)
		.opt().await? // Use opt()
		.ok_or_else(|| VotebaseError::FnNotFoundError(fn_path.clone()))?;

	// run_view needs update to accept config and pool/client appropriately
	let return_value = runtime::run_view(
		fn_path.ruleset_full_path, view_details.code, &fn_path.fn_name, &view_details.pass, query.into_inner(),
		server_pg_config, &pool, // Pass config and pool (needs refactor in run_view)
	).await?;
	Ok(web::Html::new(return_value))
}

#[cfg(debug_assertions)]
#[actix_web::post("/__debug_advance_time")]
async fn debug_advance_time(_amount: web::Json<()>) -> HttpResponse<()> {

	HttpResponse::with_body(actix_web::http::StatusCode::NO_CONTENT, ())
}
