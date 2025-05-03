#[cfg(test)]
mod test;

use std::{cell::RefCell, rc::Rc};
use deno_core::{v8, OpState};
use uuid::Uuid;
use crate::{queries, PgOpt, PgPool, PgClient, FnType, RoleType, ScheduledActionKind, format_ruleset_schema, format_ruleset_role, postgres};

pub type DenoError = deno_core::error::AnyError;

pub fn js_err<E: std::error::Error>(e: E) -> deno_error::JsErrorBox {
	deno_error::JsErrorBox::generic(e.to_string())
}

pub struct Runtime {
	js_runtime: deno_core::JsRuntime,
}

impl Runtime {
	pub async fn new(code: &str) -> Result<Self, DenoError> {
		let js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
			module_loader: None,
			startup_snapshot: Some(RUNTIME_SNAPSHOT),
			extensions: vec![votebase::init_ops()],
			..Default::default()
		});

		let mut runtime = Self { js_runtime };
		runtime.set_fn_map();
		runtime.set_external_allowed(false);

		let (code, _) = votebase_transpile::transpile_typescript(
			deno_core::ascii_str!(MAIN_SPECIFIER).into(),
			code.to_string().into(),
		)?;
		let specifier = deno_core::resolve_url(MAIN_SPECIFIER)?;
		let mod_id = runtime.js_runtime.load_main_es_module_from_code(&specifier, code).await?;
		let result = runtime.js_runtime.mod_evaluate(mod_id);
		runtime.js_runtime.run_event_loop(Default::default()).await?;
		result.await?;

		Ok(runtime)
	}

	pub fn set_fn_map(&mut self) {
		self.js_runtime.op_state().borrow_mut().put(FnMap::new());
	}
	pub fn take_fn_map(&mut self) -> FnMap {
		self.js_runtime.op_state().borrow_mut().take()
	}

	pub fn set_external_allowed(&mut self, allowed: bool) {
		self.js_runtime.op_state().borrow_mut().put(allowed);
	}

	pub fn set_fn_client(&mut self, client: PgClient) {
		self.js_runtime.op_state().borrow_mut().put(client);
	}

	pub fn set_server_opt(&mut self, opt: crate::PgOpt) {
		self.js_runtime.op_state().borrow_mut().put(ServerPgOpt(opt));
	}
	// pub fn set_fn_opt(&mut self, opt: crate::PgOpt) {
	// 	self.js_runtime.op_state().borrow_mut().put(FnPgOpt::new(opt));
	// }
	pub fn set_fn_config(&mut self, config: postgres::Config) {
		self.js_runtime.op_state().borrow_mut().put(config);
	}

	// pub fn set_pg_pool(&mut self, pool: crate::PgPool) {
	// 	self.js_runtime.op_state().borrow_mut().put(pool);
	// }

	pub fn set_current_full_path(&mut self, path: String) {
		self.js_runtime.op_state().borrow_mut().put(path);
	}
}

deno_core::extension!(
	votebase,
	ops = [
		op_fetch,
		// op_set_timeout,
		// op_sql_execute_statements,
		// op_sql_fetch_all,
		op_sql_fetch_scalar,
		// op_sql_fetch_one,
		// op_sql_fetch_optional,

		op_register_fn,
		// op_register_recurring_action,

		op_create_recurring_action,
		op_remove_recurring_action,
		op_schedule_action,
		op_unschedule_action,
		op_enroll_member,
		op_remove_member_by_email,
		op_remove_member_by_uuid,

		op_propose_self_replacement,
	],
);

static RUNTIME_SNAPSHOT: &[u8] =
	include_bytes!(concat!(env!("OUT_DIR"), "/VOTEBASE_SNAPSHOT.bin"));

const MAIN_SPECIFIER: &'static str = "votebase:<main>";


#[derive(Debug)]
struct ServerPgOpt(PgOpt);

fn demand_external_allowed(state: &RefCell<OpState>) -> Result<(), deno_error::JsErrorBox> {
	let state = state.borrow();
	let external_allowed = deno_core::_ops::opstate_borrow::<bool>(&state);
	if !external_allowed {
		return Err(deno_error::JsErrorBox::generic(ERR_EXTERNAL_NOT_ALLOWED))
	}
	Ok(())
}

#[deno_core::op2(async)]
#[string]
async fn op_fetch(
	state: Rc<RefCell<OpState>>,
	#[string] url: String,
) -> Result<String, deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;

	let body = reqwest::get(url).await.map_err(js_err)?.text().await.map_err(js_err)?;
	Ok(body)
}

// #[deno_core::op2(async)]
// async fn op_set_timeout(
// 	state: Rc<RefCell<OpState>>,
// 	delay: f64
// ) -> Result<(), deno_error::JsErrorBox> {
// 	demand_external_allowed(state.as_ref())?;
// 	// TODO pretty important to limit these timeouts
// 	// perhaps don't even allow this? all asynchrony in votebase should occur through scheduled events?

// 	tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
// 	Ok(())
// }


const ERR_EXTERNAL_NOT_ALLOWED: &'static str = "runtime functions that interact with timers or the outside world (such as database or http operations) aren't allowed outside of an action or view";

// #[deno_core::op2(async)]
// async fn op_sql_execute_many(
// 	state: Rc<RefCell<OpState>>,
// 	#[string] sql: String,
// ) -> Result<u32, deno_error::JsErrorBox> {
// 	let state = state.as_ref();
// 	demand_external_allowed(state)?;
// 	let mut state = state.borrow_mut();
// 	let connection = deno_core::_ops::opstate_borrow_mut::<FnPgOpt>(std::ops::DerefMut::deref_mut(&mut state))
// 		.connect().await.map_err(js_err)?;

// 	let result = sqlx::raw_sql(&sql).execute(connection).await.map_err(js_err)?;
// 	Ok(result.rows_affected().try_into().map_err(js_err)?)
// }

// #[deno_core::op2(async)]
// async fn op_sql_execute_statements(
// 	state: Rc<RefCell<OpState>>,
// 	#[string] sql: String,
// 	#[serde] params: Option<Vec<serde_json::Value>>,
// ) -> Result<u32, deno_error::JsErrorBox> {
// 	let state = state.as_ref();
// 	demand_external_allowed(state)?;
// 	let mut state = state.borrow_mut();
// 	let connection = deno_core::_ops::opstate_borrow_mut::<FnPgOpt>(std::ops::DerefMut::deref_mut(&mut state))
// 		.connect().await.map_err(js_err)?;

// 	let args = make_args(params).map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))?;
// 	let result = sqlx::query_with(&sql, args).execute(connection).await.map_err(js_err)?;

// 	Ok(result.rows_affected().try_into().map_err(js_err)?)
// }

// #[deno_core::op2(async)]
// #[serde]
// async fn op_sql_fetch_all(
// 	state: Rc<RefCell<OpState>>,
// 	#[string] query: String,
// 	#[serde] params: Option<Vec<serde_json::Value>>,
// ) -> Result<Vec<serde_json::Value>, deno_error::JsErrorBox> {
// 	let state = state.as_ref();
// 	demand_external_allowed(state)?;
// 	let mut state = state.borrow_mut();
// 	let connection = deno_core::_ops::opstate_borrow_mut::<FnPgOpt>(std::ops::DerefMut::deref_mut(&mut state))
// 		.connect().await.map_err(js_err)?;

// 	let args = make_args(params).map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))?;
// 	let rows = sqlx::query_with(&query, args).fetch_all(connection).await.map_err(js_err)?;

// 	Ok(rows.into_iter().map(|row| {
// 		use sqlx::{Row, Column};
// 		// TODO use this everywhere
// 		// convert_unknown_pg_value(&row, column)
// 		serde_json::Value::Object(
// 			row.columns().iter()
// 				.map(|column| (column.name().to_owned(), row.get(column.ordinal()))).collect()
// 		)
// 	}).collect())
// }

#[deno_core::op2(async)]
#[serde]
async fn op_sql_fetch_scalar(
	state: Rc<RefCell<OpState>>,
	#[string] query: String,
	#[serde] params: Option<Vec<serde_json::Value>>,
) -> Result<serde_json::Value, deno_error::JsErrorBox> {
	let state = state.as_ref();
	demand_external_allowed(state)?;
	let state = state.borrow();
	// TODO the possible downside to putting a client directly in here is that we'll connect even when we don't need to
	// I'm hoping that the pooling library just solves that for us! as in no actual connection is acquired until the actual query happens
	let client = deno_core::_ops::opstate_borrow::<PgClient>(&state);

	// let params = make_params(params);
	// let params = make_args(params).map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))?;
	let row = client.query_one(&query, &[]).await.map_err(js_err)?;
	crate::convert_pg_row(row, true)
}

// #[deno_core::op2(async)]
// #[serde]
// async fn op_sql_fetch_one(
// 	state: Rc<RefCell<OpState>>,
// 	#[string] query: String,
// 	#[serde] params: Option<Vec<serde_json::Value>>,
// ) -> Result<serde_json::Value, deno_error::JsErrorBox> {
// 	let state = state.as_ref();
// 	demand_external_allowed(state)?;
// 	let mut state = state.borrow_mut();
// 	let connection = deno_core::_ops::opstate_borrow_mut::<FnPgOpt>(std::ops::DerefMut::deref_mut(&mut state))
// 		.connect().await.map_err(js_err)?;

// 	let args = make_args(params).map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))?;
// 	let row = sqlx::query_with(&query, args).fetch_one(connection).await.map_err(js_err)?;

// 	use sqlx::Row;
// 	let columns = row.columns();
// 	Ok(serde_json::Value::Object(
// 		columns.iter().map(|column| convert_unknown_pg_value(&row, column)).collect::<Result<_, _>>()?
// 	))
// }

// #[deno_core::op2(async)]
// #[serde]
// async fn op_sql_fetch_optional(
// 	state: Rc<RefCell<OpState>>,
// 	#[string] query: String,
// 	#[serde] params: Option<Vec<serde_json::Value>>,
// ) -> Result<Option<serde_json::Value>, deno_error::JsErrorBox> {
// 	let state = state.as_ref();
// 	demand_external_allowed(state)?;
// 	let mut state = state.borrow_mut();
// 	let connection = deno_core::_ops::opstate_borrow_mut::<FnPgOpt>(std::ops::DerefMut::deref_mut(&mut state))
// 		.connect().await.map_err(js_err)?;

// 	let args = make_args(params).map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))?;
// 	let row = sqlx::query_with(&query, args).fetch_optional(connection).await.map_err(js_err)?;

// 	match row {
// 		None => Ok(None),
// 		Some(row) => {
// 			use sqlx::{Row, Column};
// 			let columns = row.columns();
// 			let obj: serde_json::Map<String, serde_json::Value> =
// 				columns.iter().map(|column| (column.name().to_owned(), row.get(column.ordinal()))).collect();

// 			Ok(Some(obj.into()))
// 		},
// 	}
// }

pub type FnMap = std::collections::HashMap<String, Fn>;

#[derive(Debug)]
pub enum Fn {
	Action(v8::Global<v8::Function>),
	View(v8::Global<v8::Function>),
}

#[deno_core::op2]
fn op_register_fn(
	#[state] fn_map: &mut FnMap,
	#[string] fn_name: &str,
	// #[serde] schema: serde_json::Value,
	is_action: bool,
	#[global] func: v8::Global<v8::Function>,
) -> Result<(), deno_error::JsErrorBox> {
	// let validator = jsonschema::draft7::new(schema)?;

	// if the js side calls zodToJsonSchema and then we use a serde serializer to decode one of these:
	// https://docs.rs/jsonschema/latest/jsonschema/struct.Validator.html
	// then we've effectively demanded actions/views to type their inputs!
	let func = match is_action {
		true => Fn::Action(func),
		false => Fn::View(func),
	};

	use std::collections::hash_map::Entry;
	match fn_map.entry(fn_name.to_owned()) {
		Entry::Vacant(entry) => {
			entry.insert(func);
			Ok(())
		},
		Entry::Occupied(_) => {
			Err(deno_error::JsErrorBox::generic(format!("already a view or action with name {}", &fn_name)))
		}
	}
}

#[deno_core::op2(async)]
#[string]
async fn op_create_recurring_action(
	state: Rc<RefCell<OpState>>,
	#[string] description: String,
	#[serde] start: chrono::DateTime<chrono::Utc>,
	#[serde] recurrence_granularity: votebase_queries::types::votebase_catalog::GranularityEnum,
	recurrence_multiplier: i16,
	#[string] action_name: String,
	#[serde] action_arg: serde_json::Value,
) -> Result<String, deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;
	let state = state.as_ref().borrow();
	let server_role_pool = deno_core::_ops::opstate_borrow::<crate::PgPool>(&state);
	let current_full_path = deno_core::_ops::opstate_borrow::<String>(&state);

	let client = server_role_pool.get().await.map_err(js_err)?;
	let action = queries::scheduled::create_detached_recurring_action()
		.bind(&client, &current_full_path, &description, &start.naive_utc(), &recurrence_granularity, &recurrence_multiplier, &action_name, &action_arg)
		.one().await.map_err(js_err)?;

	// let server_pg_opt = server_role_pool.config().clone();
	queue_scheduled_action(server_role_pool.clone(), server_pg_opt, ScheduledActionKind::DetachedRecurring, action.id, action.next_scheduled_time.and_utc());

	Ok(action.id.to_string())
}

#[deno_core::op2(async)]
#[string]
async fn op_remove_recurring_action(
	state: Rc<RefCell<OpState>>,
	#[serde] scheduled_action_uuid: Uuid,
) -> Result<(), deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;
	let state = state.as_ref().borrow();
	let server_role_pool = deno_core::_ops::opstate_borrow::<crate::PgPool>(&state);

	let client = server_role_pool.get().await.map_err(js_err)?;
	queries::scheduled::remove_detached_recurring_action()
		.bind(&client, &scheduled_action_uuid).await.map_err(js_err)?;

	Ok(())
}


#[deno_core::op2(async)]
#[string]
async fn op_schedule_action(
	state: Rc<RefCell<OpState>>,
	#[string] description: String,
	#[serde] scheduled_time: chrono::DateTime<chrono::Utc>,
	#[string] action_name: String,
	#[serde] action_arg: serde_json::Value,
) -> Result<String, deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;
	let state = state.as_ref().borrow();
	let server_role_pool = deno_core::_ops::opstate_borrow::<crate::PgPool>(&state);
	let current_full_path = deno_core::_ops::opstate_borrow::<String>(&state);

	let mut client = server_role_pool.get().await.map_err(js_err)?;
	let id = queries::scheduled::create_detached_scheduled_action()
		.bind(&client, &current_full_path, &description, &scheduled_time.fixed_offset(), &action_name, &action_arg)
		.one().await.map_err(js_err)?;

	// let server_pg_opt = server_role_pool.manager().
	queue_scheduled_action(server_role_pool.clone(), server_pg_opt, ScheduledActionKind::DetachedScheduled, id, scheduled_time);

	Ok(id.to_string())
}

#[deno_core::op2(async)]
#[string]
async fn op_unschedule_action(
	state: Rc<RefCell<OpState>>,
	#[serde] scheduled_action_uuid: Uuid,
) -> Result<(), deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;
	let state = state.as_ref().borrow();
	let server_role_pool = deno_core::_ops::opstate_borrow::<crate::PgPool>(&state);

	let client = server_role_pool.get().await.map_err(js_err)?;
	queries::scheduled::remove_detached_scheduled_action()
		.bind(&client, &scheduled_action_uuid).await.map_err(js_err)?;

	Ok(())
}

#[deno_core::op2(async)]
#[string]
async fn op_enroll_member(
	state: Rc<RefCell<OpState>>,
	#[string] email: String,
) -> Result<String, deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;
	let state = state.as_ref().borrow();
	let server_role_pool = deno_core::_ops::opstate_borrow::<crate::PgPool>(&state);

	let client = server_role_pool.get().await.map_err(js_err)?;
	let id = queries::members::enroll_member()
		.bind(&client, &email)
		.one().await.map_err(js_err)?;

	// TODO perhaps at some point there's a notification email sent to this person here or something

	Ok(id.to_string())
}

#[deno_core::op2(async)]
async fn op_remove_member_by_email(
	state: Rc<RefCell<OpState>>,
	#[string] email: String,
) -> Result<(), deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;
	let state = state.as_ref().borrow();
	let server_role_pool = deno_core::_ops::opstate_borrow::<crate::PgPool>(&state);

	let client = server_role_pool.get().await.map_err(js_err)?;
	queries::members::remove_member_by_email()
		.bind(&client, &email).await.map_err(js_err)?;

	// TODO perhaps at some point there's a notification email sent to this person here or something

	Ok(())
}
#[deno_core::op2(async)]
async fn op_remove_member_by_uuid(
	state: Rc<RefCell<OpState>>,
	#[serde] member_uuid: Uuid,
) -> Result<(), deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;
	let state = state.as_ref().borrow();

	let server_role_pool = deno_core::_ops::opstate_borrow::<crate::PgPool>(&state);
	let client = server_role_pool.get().await.map_err(js_err)?;
	queries::members::remove_member_by_uuid()
		.bind(&client, &member_uuid).await.map_err(js_err)?;

	// TODO perhaps at some point there's a notification email sent to this person here or something

	Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct CandidateSelfReplacement {
	code: String,
	db_schema: String,
	db_migration: String,
}

#[deno_core::op2(async, reentrant)]
#[string]
async fn op_propose_self_replacement(
	state: Rc<RefCell<OpState>>,
	#[serde] candidate: CandidateSelfReplacement,
) -> Result<String, deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;
	let state = state.as_ref().borrow();
	let current_full_path = deno_core::_ops::opstate_borrow::<String>(&state);
	let server_pg_opt = deno_core::_ops::opstate_borrow::<ServerPgOpt>(&state);
	let (actions, views) = validate_candidate(current_full_path, &server_pg_opt.0, &candidate).await?;

	let server_role_pool = deno_core::_ops::opstate_borrow::<crate::PgPool>(&state);
	let client = server_role_pool.get().await.map_err(js_err)?;
	let candidate_uuid = queries::rulesets::insert_candidate_replacement()
		.bind(&client, &current_full_path, &actions, &views, &candidate.code, &candidate.db_schema, &candidate.db_migration)
		.one().await.map_err(js_err)?;

	Ok(candidate_uuid.to_string())
}

// TODO all of this makes me nervous for performance. the repeated connecting over and over
// it would be nice to have a separate database server for this kind of analysis?
async fn validate_candidate(
	current_full_path: &str,
	server_pg_config: &PgOpt,
	candidate: &CandidateSelfReplacement,
) -> Result<(Vec<String>, Vec<String>), deno_error::JsErrorBox> {
	let mut inner = Runtime::new(&candidate.code).await.map_err(|e| js_err(e.root_cause()))?;

	let inner_state = inner.js_runtime.op_state();
	let inner_state = inner_state.as_ref().borrow();
	let fn_map = deno_core::_ops::opstate_borrow::<FnMap>(&inner_state);
	let mut actions = vec![];
	let mut views = vec![];
	for (fn_name, fn_type) in fn_map {
		match fn_type {
			Fn::Action(_) => { actions.push(fn_name.to_owned()) },
			Fn::View(_) => { views.push(fn_name.to_owned()) },
		}
	}

	let pgschema = format_ruleset_schema(current_full_path);
	let (declared_tempdb_config, declared_dbname) = new_tempdb(&pgschema, &format!("{current_full_path}|declared"), server_pg_config)
		.await.map_err(js_err)?;
	let (actual_tempdb_config, actual_dbname) = new_tempdb(&pgschema, &format!("{current_full_path}|actual"), server_pg_config)
		.await.map_err(js_err)?;

	let result = (|| async {
		let (declared_client, declared_conn) = declared_tempdb_config.connect(postgres::NoTls).await.map_err(js_err)?;
		tokio::spawn(async move { if let Err(e) = declared_conn.await { log::error!("DB connection error: {}", e); } });
		declared_client.batch_execute(&format!(r#"create schema "{pgschema}";"#)).await.map_err(js_err)?;
		declared_client.batch_execute(&candidate.db_schema).await.map_err(js_err)?;

		let current_schema = compute_diff(&pgschema, &actual_tempdb_config, server_pg_config).await.map_err(js_err)?;
		let (actual_client, actual_conn) = actual_tempdb_config.connect(postgres::NoTls).await.map_err(js_err)?;
		tokio::spawn(async move { if let Err(e) = actual_conn.await { log::error!("DB connection error: {}", e); } });
		actual_client.batch_execute(&current_schema).await.map_err(js_err)?;
		actual_client.batch_execute(&candidate.db_migration).await.map_err(js_err)?;

		let diff = compute_diff(&pgschema, &declared_tempdb_config, &actual_tempdb_config).await.map_err(js_err)?;
		if !diff.is_empty() {
			log::error!("{}", diff);
			Err(deno_error::JsErrorBox::generic(format!("candidate for {current_full_path} has misdeclared schema")))
		}
		else { Ok(()) }
	})().await;

	let (drop_declared, drop_actual) = tokio::join!(
		async { drop_tempdb(declared_dbname, server_pg_config).await.map_err(js_err) },
		async { drop_tempdb(actual_dbname, server_pg_config).await.map_err(js_err) },
	);
	drop_declared?;
	drop_actual?;
	result?;

	Ok((actions, views))
}

const TEMP_DB_COMMENT: &'static str = "'TEMP DB CREATED BY votebase'";

async fn new_tempdb(
	pgschema: &str,
	intended_full_path: &str,
	base_config: &PgOpt,
) -> Result<(PgOpt, String), postgres::Error> {
	let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
	let intended_full_path = format_ruleset_schema(intended_full_path);
	let dbname = format!("temp_db:{intended_full_path}|{now}");

	let (client, connection) = base_config.connect(postgres::NoTls).await?;
	tokio::spawn(async move { if let Err(e) = connection.await { log::error!("DB connection error: {}", e); } });

	// TODO is it possible to do this create database in the same query?
	client.batch_execute(&format!(r#"create database "{dbname}";"#)).await?;
	client.batch_execute(&format!(r#"
		alter database "{dbname}" set search_path = '{pgschema}';
		comment on database "{dbname}" is {TEMP_DB_COMMENT};
	"#)).await?;

	let mut config = base_config.clone();
	config.dbname(&dbname);
	Ok((config, dbname))
}

async fn drop_tempdb(dbname: String, base_config: &PgOpt) -> Result<(), postgres::Error> {
	let (client, connection) = base_config.connect(postgres::NoTls).await?;
	tokio::spawn(async move { if let Err(e) = connection.await { log::error!("DB connection error: {}", e); } });

	client.batch_execute(&format!(r#"drop database if exists "{dbname}";"#)).await?;
	Ok(())
}

async fn compute_diff(
	pgschema: &str,
	current_config: &PgOpt,
	intended_config: &PgOpt,
) -> Result<String, deno_error::JsErrorBox> {
	#[cfg(debug_assertions)]
	let mut command = {
		let mut command = tokio::process::Command::new("uv");
		command.args("tool run -p 3.11 --with psycopg2-binary --with setuptools migra".split_whitespace());
		command
	};
	#[cfg(not(debug_assertions))]
	let mut command = tokio::process::Command::new("migra");

	let output = command
		.arg("--unsafe")
		.arg("--with-privileges")
		.arg("--schema").arg(pgschema)
		.arg(convert_db_url(current_config))
		.arg(convert_db_url(intended_config))
		.output()
		.await
		.map_err(js_err)?;

	if !output.stderr.is_empty() {
		let e = format!("migra failed: {}\n\n{}", output.status, String::from_utf8_lossy(&output.stderr));
		return Err(deno_error::JsErrorBox::generic(e));
	}
	Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn convert_db_url(url: &PgOpt) -> String {
	url.into()
	// let mut url = url.to_url_lossy();
	// url.set_scheme("postgresql").unwrap();
	// url.set_query(None);
	// url.to_string()
}

// pub async fn clean_all_temp_dbs(base_config: &PgOpt) -> Result<(), postgres::Error> {
// 	let (mut client, connection) = base_config.connect(postgres::NoTls).await?;
//  tokio::spawn(async move { if let Err(e) = connection.await { log::error!("DB connection error: {}", e); } });
// 	let temp_dbs = client.query(
// 		r#"select datname from pg_database where obj_description(oid, 'pg_database') = $1;"#,
// 		TEMP_DB_COMMENT,
// 	)
// 	.await?;

// 	for row in temp_dbs {
//    let datname: String = row.get(0);
// 		if let Err(e) = client.batch_execute(&format!(r#"drop database if exists "{}";"#, datname))
// 			.await
// 		{
// 			eprintln!("Failed to drop temporary database {}: {}", row.datname, e);
// 		}
// 	}

// 	Ok(())
// }

pub async fn replace_ruleset(
	server_client: &PgClient,
	migrator_role_config: PgOpt,
	new_ruleset_id: Uuid,
) -> Result<(), postgres::Error> {
	let db_migration = queries::rulesets::apply_candidate().bind(server_client, &new_ruleset_id).one().await?;

	let (migrator_client, migrator_connection) = migrator_role_config.connect(postgres::NoTls).await?;
	tokio::spawn(async move { if let Err(e) = migrator_connection.await { log::error!("DB connection error: {}", e); } });
	migrator_client.batch_execute(&db_migration).await?;
	Ok(())
}

pub async fn create_ruleset(
	client: &PgClient,
	parent_full_path: Option<&str>, name: &str,
	action_names: &Vec<String>, view_names: &Vec<String>,
	ruleset_code: &str, db_schema: &str,
) -> Result<(), postgres::Error> {
	let transaction = client.transaction().await?;

	let ruleset_row = queries::rulesets::insert_ruleset()
		.bind(client, &parent_full_path, &name, &action_names, &view_names, &ruleset_code, &db_schema).one().await?;

	let full_path = ruleset_row.full_path;

	let formatted_ruleset_role_migrator = format_ruleset_role(&full_path, RoleType::Migrator);
	let formatted_ruleset_role_action = format_ruleset_role(&full_path, RoleType::Action);
	let formatted_ruleset_role_view = format_ruleset_role(&full_path, RoleType::View);

	let create_sql = format!(include_str!("./create-ruleset.sql"),
		formatted_ruleset_schema=format_ruleset_schema(&full_path),
		formatted_ruleset_role_migrator=formatted_ruleset_role_migrator,
		formatted_ruleset_role_action=formatted_ruleset_role_action,
		formatted_ruleset_role_view=formatted_ruleset_role_view,
		migrator_pass=ruleset_row.migrator_pass,
		action_pass=ruleset_row.action_pass,
		view_pass=ruleset_row.view_pass,
	);
	transaction.batch_execute(&create_sql).await?;
	transaction.commit().await?;

	// let base_config = pool.config();
	let mut migrator_config = base_config.clone();
	migrator_config.user(&formatted_ruleset_role_migrator);
	migrator_config.password(Some(ruleset_row.migrator_pass));

	// Connect using postgres::connect
	let (mut migrator_client, migrator_connection) = migrator_config.connect(postgres::NoTls).await?;
	tokio::spawn(async move { if let Err(e) = migrator_connection.await { log::error!("DB connection error: {}", e); } });
	migrator_client.batch_execute(db_schema).await?;

	Ok(())
}

fn compute_time_until(scheduled_time: chrono::DateTime<chrono::Utc>) -> tokio::time::Instant {
	let duration_until = scheduled_time.signed_duration_since(chrono::Utc::now()).to_std().unwrap();
	tokio::time::Instant::now() + duration_until
}

pub fn queue_scheduled_action(
	server_role_pool: PgPool,
	server_pg_opt: PgOpt,
	scheduled_action_kind: ScheduledActionKind,
	scheduled_action_uuid: Uuid,
	scheduled_time: chrono::DateTime<chrono::Utc>,
) {
	let scheduled_time = compute_time_until(scheduled_time);

	tokio::task::spawn_local(async move {
		tokio::time::sleep_until(scheduled_time).await;

		log::info!("attempting action {} of type {}", scheduled_action_uuid, &scheduled_action_kind);
		let result = match scheduled_action_kind {
			// ScheduledActionKind::Recurring => {
			// 	execute_recurring_action(server_role_pool, server_pg_opt, false, scheduled_action_uuid).await
			// },
			ScheduledActionKind::DetachedRecurring => {
				execute_recurring_action(server_role_pool, server_pg_opt, scheduled_action_uuid).await
			},
			ScheduledActionKind::DetachedScheduled => {
				execute_scheduled_action(server_role_pool, server_pg_opt, scheduled_action_uuid).await
			},
		};

		if let Err(e) = result {
			error!("failed to execute scheduled action {}; {}", scheduled_action_uuid, e);
		}
	});
}

#[derive(Debug)]
struct ExecutableRecurringAction {
	code: String,
	action_pass: String,
	migrator_pass: String,
	description: String,
	next_scheduled_time: chrono::NaiveDateTime,
	full_path: String,
	action_name: String,
	arg: serde_json::Value,
}

pub async fn execute_recurring_action(
	server_role_pool: PgPool,
	server_pg_opt: PgOpt,
	// is_detached: bool,
	scheduled_action_uuid: uuid::Uuid,
) -> Result<(), DenoError> {
	// let scheduled_action_kind = if is_detached { ScheduledActionKind::DetachedRecurring } else { ScheduledActionKind::Recurring };
	let scheduled_action_kind = ScheduledActionKind::DetachedRecurring;

	let action = queries::scheduled::acquire_detached_recurring_action()
		.bind(&server_role_pool, &scheduled_action_uuid).opt().await?;
	// if is_detached {
	// } else {
	// 	unimplemented!()
	// };

	match action {
		None => { info!("wasn't able to acquire scheduled action {}", scheduled_action_uuid); Ok(()) },
		Some(action) => {
			let scheduled = action.next_scheduled_time.and_utc();
			let now = chrono::Utc::now();
			let diff = scheduled - now;
			log::info!("{} scheduled: {}; actual: {}; difference: {}; {}", scheduled_action_uuid, scheduled, now, diff, action.description);
			if diff < ::chrono::TimeDelta::zero() {
				log::info!("{} not doing it yet", scheduled_action_uuid);
				queue_scheduled_action(server_role_pool, server_pg_opt, scheduled_action_kind, scheduled_action_uuid, scheduled);
				return Ok(())
			}

			run_action(
				action.full_path, action.code, &action.action_name, &action.action_pass, &action.migrator_pass, action.arg,
				server_pg_opt.clone(), &server_role_pool,
			).await?;

			let next_scheduled_time = queries::scheduled::release_detached_recurring_action()
				.bind(&server_role_pool, &scheduled_action_uuid).one().await?.and_utc();
			// if is_detached {
			// } else {
			// 	unimplemented!()
				// sqlx::query!(r#"
				// 	update votebase_catalog.recurring_action
				// 	set executing = false, executed_count = executed_count + 1
				// 	where id = $1
				// 	returning next_scheduled_time;
				// "#, &scheduled_action_uuid).fetch_one(&server_role_pool).await?.next_scheduled_time.and_utc()
			// };

			queue_scheduled_action(server_role_pool, server_pg_opt, scheduled_action_kind, scheduled_action_uuid, next_scheduled_time);

			Ok(())
		},
	}
}

pub async fn execute_scheduled_action(
	server_role_pool: crate::PgPool,
	server_pg_opt: crate::PgOpt,
	scheduled_action_uuid: uuid::Uuid,
) -> Result<(), DenoError> {
	let client = server_role_pool.get().await.map_err(js_err)?;
	let action = queries::scheduled::acquire_detached_scheduled_action()
		.bind(&client, &scheduled_action_uuid)
		.opt().await.map_err(js_err)?;

	match action {
		None => { info!("wasn't able to acquire scheduled action {}", scheduled_action_uuid); Ok(()) },
		Some(action) => {
			let scheduled = action.scheduled_time;
			let now = chrono::Utc::now();
			let diff = scheduled - now;
			log::info!("{} ({}): scheduled: {}; actual: {}; difference: {}", scheduled_action_uuid, action.description, scheduled, now, diff);

			run_action(
				action.full_path, action.code, &action.action_name, &action.action_pass, &action.migrator_pass, action.arg,
				server_pg_opt.clone(), &server_role_pool,
			).await?;

			queries::scheduled::remove_detached_scheduled_action()
				.bind(&client, &scheduled_action_uuid).await.map_err(js_err)?;

			Ok(())
		},
	}
}

pub async fn run_action(
	current_full_path: String,
	ruleset_code: String,
	action_name: &str,
	action_pass: &str,
	migrator_pass: &str,
	arg: serde_json::Value,
	mut action_role_url: postgres::Config,
	server_pg_opt: PgOpt,
	server_role_pool: &crate::PgPool,
) -> Result<(), DenoError> {
	let migrator_role = format_ruleset_role(&current_full_path, RoleType::Migrator);
	let mut migrator_role_url = action_role_url.clone();
	migrator_role_url.user(&migrator_role).password(migrator_pass);
	let action_role = format_ruleset_role(&current_full_path, RoleType::Action);
	action_role_url.user(&action_role).password(action_pass);

	let new_ruleset_id = run_function::<Option<String>>(
		current_full_path, ruleset_code, action_name, arg, FnType::Action,
		action_role_url, server_pg_opt, server_role_pool.clone(),
	).await?;

	if let Some(new_ruleset_id) = new_ruleset_id {
		let new_ruleset_id = new_ruleset_id.parse::<uuid::Uuid>()?;
		info!("apply_candidate {new_ruleset_id}");
		replace_ruleset(&server_role_pool, migrator_role_url, new_ruleset_id).await?;
	}

	Ok(())
}

pub async fn run_view(
	current_full_path: String,
	ruleset_code: String,
	view_name: &str,
	view_pass: &str,
	query: serde_json::Value,
	mut view_role_url: postgres::Config,
	server_pg_opt: PgOpt,
	server_role_pool: &crate::PgPool,
) -> Result<String, DenoError> {
	let view_role = format_ruleset_role(&current_full_path, RoleType::View);
	view_role_url.user(view_role).password(view_pass);

	run_function(
		current_full_path, ruleset_code, view_name, query, FnType::View,
		view_role_url, server_pg_opt, server_role_pool.clone(),
	).await
}

async fn run_function<'r, V: deno_core::serde::Deserialize<'r>>(
	current_full_path: String,
	ruleset_code: String,
	function_name: &str,
	function_arg: serde_json::Value,
	function_type: FnType,
	fn_role_url: postgres::Config,
	server_pg_opt: PgOpt,
	server_role_pool: crate::PgPool,
) -> Result<V, DenoError> {
	// running a function could do a variety of things:
	// - a mere view
	// - an action, which can create new rulesets, replace the current one, etc
	// for many runtime abilities the server is acting in it's capacity

	let mut runtime = Runtime::new(&ruleset_code).await?;

	// fn_role_url encodes the user, and therefore the role and powers of the connection
	let fn_map = runtime.take_fn_map();
	let function = fn_map.get(function_name)
		.ok_or_else(|| deno_core::anyhow::anyhow!("{} '{}' not found", function_type, function_name))?;

	let function = match (function_type, function) {
		(FnType::Action, Fn::Action(function)) => function,
		(FnType::View, Fn::View(function)) => function,
		_ => { return Err(deno_core::anyhow::anyhow!("'{}' isn't of type {}", function_name, function_type)) },
	};

	let function_arg = {
		let mut scope = runtime.js_runtime.handle_scope();
		let function_arg = deno_core::serde_v8::to_v8(&mut scope, function_arg)?;
		v8::Global::new(&mut scope, function_arg)
	};

	runtime.set_external_allowed(true);
	runtime.set_server_opt(server_pg_opt);
	runtime.set_pg_pool(server_role_pool);
	runtime.set_fn_config(fn_role_url);
	runtime.set_current_full_path(current_full_path);

	// TODO also pass user_id here, maybe with some other context in the future
	let call = runtime.js_runtime.call_with_args(function, &[function_arg]);
	let call_return_value = runtime.js_runtime
		.with_event_loop_promise(call, deno_core::PollEventLoopOptions::default())
		.await?;

	let mut scope = runtime.js_runtime.handle_scope();
	let call_return_value = v8::Local::new(&mut scope, call_return_value);
	Ok(deno_core::serde_v8::from_v8(&mut scope, call_return_value)?)
}
