use votebase_common::{deadpool, postgres, queries, runtime, PgConfig, PgPool};

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

	let mut server_pg_config = postgres::Config::new();
	server_pg_config
		.port(db_port)
		.host(&db_host)
		.dbname(&db_database)
		.user(db_votebase_user)
		.password(&db_votebase_pass);

	let max_connections = std::env::var("DATABASE_MAX_CONNECTIONS").ok().and_then(|s| s.parse().ok()).unwrap_or(5);

	let pool = deadpool::Pool::builder(deadpool::Manager::new(server_pg_config.clone(), postgres::NoTls))
		.max_size(max_connections as usize)
		.build().expect("Failed to create pool.");
	let scheduled_action_queue = runtime::ScheduledActionQueue::new();

	let queue_pool = pool.clone();
	let queue_scheduled_action_queue = scheduled_action_queue.clone();
	let queue_server_pg_config = server_pg_config.clone();
	tokio::task::spawn(async move {
		let client = match queue_pool.get().await {
			Ok(client) => client,
			Err(e) => {
				log::error!("Failed to get client for queueing background tasks: {}", e);
				return;
			}
		};

		match queries::scheduled::select_slim_detached_scheduled_actions().bind(&client).all().await {
			Err(e) => log::error!("failed to fetch detached_scheduled_actions: {}", e),
			Ok(detached_scheduled_actions) => {
				log::info!("queuing {} detached_scheduled_actions", detached_scheduled_actions.len());
				for action in detached_scheduled_actions {
					queue_scheduled_action_queue.queue(
						queue_pool.clone(), queue_server_pg_config.clone(), votebase_common::ScheduledActionKind::DetachedScheduled,
						action.id, action.scheduled_time.to_utc(),
					);
				}
			},
		}

		match queries::scheduled::select_slim_detached_recurring_actions().bind(&client).all().await {
			Err(e) => log::error!("failed to fetch detached_recurring_actions: {}", e),
			Ok(detached_recurring_actions) => {
				log::info!("queuing {} detached_recurring_actions", detached_recurring_actions.len());
				for action in detached_recurring_actions {
					queue_scheduled_action_queue.queue(
						queue_pool.clone(), queue_server_pg_config.clone(), votebase_common::ScheduledActionKind::DetachedRecurring,
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
			.app_data(web::Data::new(server_pg_config.clone()))
			.app_data(web::Data::new(scheduled_action_queue.clone()))
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


#[actix_web::get("/rulesets")]
async fn get_rulesets(
	pool: web::Data<PgPool>,
) -> Result<web::Json<Vec<String>>, VotebaseError> {
	let client = pool.get().await?;

	let rulesets = queries::rulesets::get_rulesets()
		.bind(&client)
		.all().await?;

	Ok(web::Json(rulesets))
}

#[actix_web::get("/ruleset-views/{ruleset_full_path}")]
async fn get_ruleset_views(
	ruleset_full_path: web::Path<String>,
	pool: web::Data<PgPool>,
) -> Result<web::Json<Vec<String>>, VotebaseError> {
	let ruleset_full_path = ruleset_full_path.into_inner();
	let client = pool.get().await?;

	let ruleset = queries::rulesets::get_ruleset_views()
		.bind(&client, &ruleset_full_path)
		.opt().await?
		.ok_or_else(|| VotebaseError::RulesetNotFoundError(ruleset_full_path))?;

	Ok(web::Json(ruleset))
}

#[actix_web::get("/ruleset-detail/{ruleset_full_path}")]
async fn get_ruleset_detail(
	ruleset_full_path: web::Path<String>,
	pool: web::Data<PgPool>,
) -> Result<web::Json<queries::rulesets::GetRulesetDetail>, VotebaseError> {
	let ruleset_full_path = ruleset_full_path.into_inner();
	let client = pool.get().await?;

	let ruleset = queries::rulesets::get_ruleset_detail()
		.bind(&client, &ruleset_full_path)
		.opt().await?
		.ok_or_else(|| VotebaseError::RulesetNotFoundError(ruleset_full_path))?;
	// TODO	have to look at this

	Ok(web::Json(ruleset))
}

#[actix_web::post("/fn/action/{path}")]
async fn execute_action(
	fn_path: FnPath,
	arg: web::Json<serde_json::Value>,
	pool: web::Data<PgPool>,
	server_pg_config: web::Data<PgConfig>,
	scheduled_action_queue: web::Data<runtime::ScheduledActionQueue>,
) -> Result<HttpResponse<()>, VotebaseError> {
	let client = pool.get().await?;
	let server_pg_config = server_pg_config.get_ref().clone();
	let scheduled_action_queue = scheduled_action_queue.get_ref().clone();

	let action_details = queries::rulesets::get_action_details()
		.bind(&client, &fn_path.ruleset_full_path, &fn_path.fn_name)
		.opt().await?
		.ok_or_else(|| VotebaseError::FnNotFoundError(fn_path.clone()))?;

	runtime::run_action(
		fn_path.ruleset_full_path, action_details.code, &fn_path.fn_name,
		&action_details.action_pass, &action_details.migrator_pass, arg.into_inner(),
		server_pg_config, &pool, scheduled_action_queue,
	).await?;

	Ok(HttpResponse::with_body(actix_web::http::StatusCode::NO_CONTENT, ()))
}

#[actix_web::get("/fn/view/{path}")]
async fn execute_view(
	fn_path: FnPath,
	query: web::Query<serde_json::Value>,
	pool: web::Data<PgPool>,
	server_pg_config: web::Data<PgConfig>,
	scheduled_action_queue: web::Data<runtime::ScheduledActionQueue>,
) -> Result<web::Html, VotebaseError> {
	let client = pool.get().await?;
	let server_pg_config = server_pg_config.get_ref().clone();
	let scheduled_action_queue = scheduled_action_queue.get_ref().clone();

	let view_details = queries::rulesets::get_view_details()
		.bind(&client, &fn_path.ruleset_full_path, &fn_path.fn_name)
		.opt().await?
		.ok_or_else(|| VotebaseError::FnNotFoundError(fn_path.clone()))?;

	let return_value = runtime::run_view(
		fn_path.ruleset_full_path, view_details.code, &fn_path.fn_name, &view_details.pass, query.into_inner(),
		server_pg_config, &pool, scheduled_action_queue,
	).await?;
	Ok(web::Html::new(return_value))
}

#[cfg(debug_assertions)]
#[actix_web::post("/__debug_advance_time")]
async fn debug_advance_time(_amount: web::Json<()>) -> HttpResponse<()> {
	// placeholder...
	HttpResponse::with_body(actix_web::http::StatusCode::NO_CONTENT, ())
}
