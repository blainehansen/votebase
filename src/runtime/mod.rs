#[cfg(test)]
mod test;

use std::{cell::RefCell, rc::Rc};
use deno_core::{v8, OpState};
use sqlx::Connection;
use crate::PgOpt;

pub type DenoError = deno_core::error::AnyError;

pub fn js_err<E: std::error::Error>(e: E) -> deno_error::JsErrorBox {
	deno_error::JsErrorBox::generic(e.to_string())
}

pub struct Runtime {
	js_runtime: deno_core::JsRuntime,
}

// there are a couple of roles that can be used
// - the roles for actions and views, which are created at runtime in the course of creating rulesets. these are very weak and only able to access their own ruleset schema and whatever permissions are given to them by their parent ruleset
// - the server role, which has the ability to read and mutate the catalog, and by extension
// - another even stronger role used for checking migrations? since it needs to create databases? is this scary? it might not be, since the ability to create a database just means you can play with *that* database, not others
struct ServerPgOpt(PgOpt);
struct FnPgOpt {
	opt: PgOpt,
	connection: Option<sqlx::postgres::PgConnection>,
}

impl FnPgOpt {
	fn new(opt: PgOpt) -> FnPgOpt {
		FnPgOpt { opt, connection: None }
	}
	async fn connect(&mut self) -> Result<&mut sqlx::postgres::PgConnection, sqlx::Error> {
		if self.connection.is_none() {
			self.connection = Some(sqlx::postgres::PgConnection::connect_with(&self.opt.options).await?);
		}
		Ok(self.connection.as_mut().unwrap())
	}
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

		let (code, _) = transpile_helpers::transpile_typescript(
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

	pub fn set_server_opt(&mut self, opt: crate::PgOpt) {
		self.js_runtime.op_state().borrow_mut().put(ServerPgOpt(opt));
	}
	pub fn set_fn_opt(&mut self, opt: crate::PgOpt) {
		self.js_runtime.op_state().borrow_mut().put(FnPgOpt::new(opt));
	}

	pub fn set_pg_pool(&mut self, pool: crate::PgPool) {
		self.js_runtime.op_state().borrow_mut().put(pool);
	}

	pub fn set_current_full_path(&mut self, path: String) {
		self.js_runtime.op_state().borrow_mut().put(path);
	}
}

deno_core::extension!(
	votebase,
	ops = [
		op_fetch,
		op_set_timeout,
		op_sql_execute_many,

		op_register_fn,
		op_propose_self_replacement,
	],
);

static RUNTIME_SNAPSHOT: &[u8] =
	include_bytes!(concat!(env!("OUT_DIR"), "/VOTEBASE_SNAPSHOT.bin"));

const MAIN_SPECIFIER: &'static str = "votebase:<main>";


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

#[deno_core::op2(async)]
async fn op_set_timeout(
	state: Rc<RefCell<OpState>>,
	delay: f64
) -> Result<(), deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;

	tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
	Ok(())
}


const ERR_EXTERNAL_NOT_ALLOWED: &'static str = "runtime functions that interact with timers or the outside world (such as database or http operations) aren't allowed outside the context of an action or view";

#[deno_core::op2(async)]
async fn op_sql_execute_many(
	state: Rc<RefCell<OpState>>,
	#[string] sql: String,
) -> Result<u32, deno_error::JsErrorBox> {
	let state = state.as_ref();
	demand_external_allowed(state)?;
	let mut state = state.borrow_mut();

	let connection = deno_core::_ops::opstate_borrow_mut::<sqlx::PgConnection>(std::ops::DerefMut::deref_mut(&mut state));
	let result = sqlx::raw_sql(&sql).execute(connection).await.map_err(js_err)?;
	Ok(result.rows_affected().try_into().map_err(js_err)?)
}

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
	is_action: bool,
	#[global] func: v8::Global<v8::Function>,
) -> Result<(), deno_error::JsErrorBox> {
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
	let server_opt = deno_core::_ops::opstate_borrow::<PgOpt>(&state);
	let (actions, views) = validate_candidate(current_full_path, server_opt, &candidate).await?;

	let server_role_pool = deno_core::_ops::opstate_borrow::<crate::PgPool>(&state);
	let candidate_uuid = sqlx::query!(
		r#"select u as "candidate_uuid!" from votebase_catalog.insert_candidate_replacement($1, $2, $3, $4, $5, $6) as t(u);"#,
		&current_full_path, &actions, &views, &candidate.code, &candidate.db_schema, &candidate.db_migration,
	).fetch_one(server_role_pool).await.map_err(js_err)?.candidate_uuid;

	Ok(candidate_uuid.into())
}

fn ruleset_pgschema(full_path: &str) -> String {
	let full_path = full_path.split(crate::PATH_DELIMITER).collect::<Vec<&str>>().join("_");
	format!("votebase_ruleset_{full_path}")
}

async fn validate_candidate(
	current_full_path: &str,
	server_opt: &PgOpt,
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

	let output = tokio::process::Command::new("pg_dump")
		.arg("--schema-only")
		.arg(format!("--dbname={}", &server_opt.database))
		.arg(format!("--schema={}", ruleset_pgschema(current_full_path)))
		.arg("--username").arg(server_opt.options.get_username())
		.arg("--host").arg(server_opt.options.get_host())
		.arg("--port").arg(server_opt.options.get_port().to_string())
		.env("PGPASSWORD", &server_opt.password)
		.output()
		.await.map_err(js_err)?;

	if !output.status.success() {
		return Err(deno_error::JsErrorBox::generic(format!(
			"pg_dump failed: {}\n{}",
			output.status,
			String::from_utf8_lossy(&output.stderr)
		)));
	}

	let current_schema = String::from_utf8_lossy(&output.stdout).to_string();
	// TODO if either one of these fails to create we need to drop the other if it succeeded
	let (declared_tempdb, actual_tempdb) = tokio::try_join!(
		async { new_tempdb(&format!("{current_full_path}_declared"), &server_opt).await.map_err(js_err) },
		async { new_tempdb(&format!("{current_full_path}_actual"), &server_opt).await.map_err(js_err) },
	)?;

	let result = (|| async {
		let mut declared_conn = sqlx::PgConnection::connect_with(&declared_tempdb.options).await.map_err(js_err)?;
		sqlx::raw_sql(&candidate.db_schema).execute(&mut declared_conn).await.map_err(js_err)?;

		let mut actual_conn = sqlx::PgConnection::connect_with(&actual_tempdb.options).await.map_err(js_err)?;
		sqlx::raw_sql(&current_schema).execute(&mut actual_conn).await.map_err(js_err)?;
		sqlx::raw_sql(&candidate.db_migration).execute(&mut actual_conn).await.map_err(js_err)?;

		let diff = compute_diff(&declared_tempdb, &actual_tempdb).await.map_err(js_err)?;
		if !diff.is_empty() {
			Err(deno_error::JsErrorBox::generic(format!("candidate for {current_full_path} has misdeclared schema")))
		}
		else { Ok(()) }
	})().await;

	let (drop_declared, drop_actual) = tokio::join!(
		async { drop_tempdb(declared_tempdb.database, &server_opt.options).await.map_err(js_err) },
		async { drop_tempdb(actual_tempdb.database, &server_opt.options).await.map_err(js_err) },
	);
	drop_declared?;
	drop_actual?;
	result?;

	Ok((actions, views))
}

fn to_connection_string(config: &PgOpt) -> String {
	let username = config.options.get_username();
	let password = &config.password;
	let host = config.options.get_host();
	let port = config.options.get_port();
	let dbname = config.options.get_database().unwrap_or("");
	format!("postgresql://{username}:{password}@{host}:{port}/{dbname}")
}

async fn compute_diff(
	current: &PgOpt,
	intended: &PgOpt,
) -> Result<String, deno_error::JsErrorBox> {
	let output = tokio::process::Command::new("migra")
		.arg("--unsafe")
		.arg("--with-privileges")
		.arg("--exclude_schema").arg("votebase_catalog")
		.arg(to_connection_string(current))
		.arg(to_connection_string(intended))
		.output()
		.await
		.map_err(js_err)?;

	if !output.stderr.is_empty() {
		let e = format!("migra failed: {}\n\n{}", output.status, String::from_utf8_lossy(&output.stderr));
		return Err(deno_error::JsErrorBox::generic(e));
	}
	Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}


const TEMP_DB_COMMENT: &'static str = "'TEMP DB CREATED BY votebase'";

async fn new_tempdb(
	intended_full_path: &str,
	base_config: &PgOpt,
) -> Result<PgOpt, sqlx::Error> {
	let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
	let intended_full_path = ruleset_pgschema(intended_full_path);
	let dbname = format!("___votebase_temp__{intended_full_path}__{now}");

	let config = base_config.clone().database(&dbname);
	let mut conn = sqlx::PgConnection::connect_with(&config.options).await?;
	sqlx::raw_sql(&format!(r#"
		create database "{dbname}";
		comment on database "{dbname}" is {TEMP_DB_COMMENT};
	"#)).execute(&mut conn).await?;

	Ok(config)
}

async fn drop_tempdb(dbname: String, base_config: &sqlx::postgres::PgConnectOptions) -> Result<(), sqlx::Error> {
	let mut conn = sqlx::PgConnection::connect_with(base_config).await?;

	sqlx::raw_sql(&format!(r#"drop database if exists "{dbname}";"#))
		.execute(&mut conn).await?;

	Ok(())
}

// pub async fn clean_all_temp_dbs(base_config: &sqlx::postgres::PgConnectOptions) -> Result<(), sqlx::Error> {
// 	let mut conn = sqlx::PgConnection::connect_with(base_config).await?;

// 	let temp_dbs = sqlx::query!(
// 		r#"select datname from pg_database where obj_description(oid, 'pg_database') = $1;"#,
// 		TEMP_DB_COMMENT,
// 	)
// 	.fetch_all(&mut conn)
// 	.await?;

// 	for row in temp_dbs {
// 		if let Err(e) = sqlx::raw_sql(&format!(r#"drop database if exists "{}";"#, row.datname))
// 			.execute(&mut conn)
// 			.await
// 		{
// 			eprintln!("Failed to drop temporary database {}: {}", row.datname, e);
// 		}
// 	}

// 	Ok(())
// }

pub async fn replace_ruleset(
	pool: &crate::PgPool,
	new_ruleset_id: sqlx::types::Uuid,
) -> Result<(), sqlx::Error> {
	let mut tx = pool.begin().await?;

	let db_migration = sqlx::query!(
		r#"select m as "db_migration!" from votebase_catalog.apply_candidate($1) as t(m);"#,
		new_ruleset_id,
	).fetch_one(&mut *tx).await?.db_migration;

	sqlx::raw_sql(&db_migration).execute(&mut *tx).await?;

	tx.commit().await
}

#[derive(Copy, Clone, Debug)]
pub enum FnType { Action, View }
impl std::fmt::Display for FnType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			FnType::Action => write!(f, "action"),
			FnType::View => write!(f, "view"),
		}
	}
}

pub async fn run_function<'r, V: deno_core::serde::Deserialize<'r>>(
	current_full_path: String,
	ruleset_code: String,
	function_name: &str,
	function_arg: serde_json::Value,
	function_type: FnType,
	fn_role_url: &sqlx::postgres::PgConnectOptions,
	server_role_pool: crate::PgPool,
) -> Result<V, DenoError> {
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
	runtime.set_pg_connection(fn_role_url).await?;
	runtime.set_pg_pool(server_role_pool);
	runtime.set_current_full_path(current_full_path);

	let call = runtime.js_runtime.call_with_args(function, &[function_arg]);
	let call_return_value = runtime.js_runtime
		.with_event_loop_promise(call, deno_core::PollEventLoopOptions::default())
		.await?;

	let mut scope = runtime.js_runtime.handle_scope();
	let call_return_value = v8::Local::new(&mut scope, call_return_value);
	Ok(deno_core::serde_v8::from_v8(&mut scope, call_return_value)?)
}
