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


#[derive(Debug)]
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
			self.connection = Some(sqlx::postgres::PgConnection::connect_with(&self.opt).await?);
		}
		Ok(self.connection.as_mut().unwrap())
	}
}


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
	let connection = deno_core::_ops::opstate_borrow_mut::<FnPgOpt>(std::ops::DerefMut::deref_mut(&mut state))
		.connect().await.map_err(js_err)?;

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
	let server_opt = deno_core::_ops::opstate_borrow::<ServerPgOpt>(&state);
	let (actions, views) = validate_candidate(current_full_path, &server_opt.0, &candidate).await?;

	let server_role_pool = deno_core::_ops::opstate_borrow::<crate::PgPool>(&state);
	let candidate_uuid = sqlx::query!(
		r#"select u as "candidate_uuid!" from votebase_catalog.insert_candidate_replacement($1, $2, $3, $4, $5, $6) as t(u);"#,
		&current_full_path, &actions, &views, &candidate.code, &candidate.db_schema, &candidate.db_migration,
	).fetch_one(server_role_pool).await.map_err(js_err)?.candidate_uuid;

	Ok(candidate_uuid.into())
}

fn ruleset_pgschema(full_path: &str) -> String {
	format!("ruleset:{full_path}")
}

async fn validate_candidate(
	current_full_path: &str,
	server_opt: &sqlx::postgres::PgConnectOptions,
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

	let pgschema = ruleset_pgschema(current_full_path);
	let (declared_tempdb, declared_dbname) = new_tempdb(&pgschema, &format!("{current_full_path}_declared"), &server_opt).await.map_err(js_err)?;
	let (actual_tempdb, actual_dbname) = new_tempdb(&pgschema, &format!("{current_full_path}_actual"), &server_opt).await.map_err(js_err)?;

	let result = (|| async {
		let mut declared_conn = sqlx::PgConnection::connect_with(&declared_tempdb).await.map_err(js_err)?;
		sqlx::raw_sql(&format!(r#"
			create schema "{pgschema}";
		"#)).execute(&mut declared_conn).await.map_err(js_err)?;
		sqlx::raw_sql(&candidate.db_schema).execute(&mut declared_conn).await.map_err(js_err)?;

		let current_schema = compute_diff(&pgschema, &actual_tempdb, &server_opt).await.map_err(js_err)?;
		let mut actual_conn = sqlx::PgConnection::connect_with(&actual_tempdb).await.map_err(js_err)?;
		sqlx::raw_sql(&current_schema).execute(&mut actual_conn).await.map_err(js_err)?;
		sqlx::raw_sql(&candidate.db_migration).execute(&mut actual_conn).await.map_err(js_err)?;

		let diff = compute_diff(&pgschema, &declared_tempdb, &actual_tempdb).await.map_err(js_err)?;
		if !diff.is_empty() {
			log::error!("{}", diff);
			Err(deno_error::JsErrorBox::generic(format!("candidate for {current_full_path} has misdeclared schema")))
		}
		else { Ok(()) }
	})().await;

	let (drop_declared, drop_actual) = tokio::join!(
		async { drop_tempdb(declared_dbname, &server_opt).await.map_err(js_err) },
		async { drop_tempdb(actual_dbname, &server_opt).await.map_err(js_err) },
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
	base_config: &sqlx::postgres::PgConnectOptions,
) -> Result<(sqlx::postgres::PgConnectOptions, String), sqlx::Error> {
	let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
	let intended_full_path = ruleset_pgschema(intended_full_path);
	let dbname = format!("___votebase_temp__{intended_full_path}__{now}");

	let mut conn = sqlx::PgConnection::connect_with(&base_config).await?;
	sqlx::raw_sql(&format!(r#"
		create database "{dbname}";
	"#)).execute(&mut conn).await?;
	sqlx::raw_sql(&format!(r#"
		alter database "{dbname}" set search_path = '{pgschema}';
		comment on database "{dbname}" is {TEMP_DB_COMMENT};
	"#)).execute(&mut conn).await?;

	let config = base_config.clone().database(&dbname);
	Ok((config, dbname))
}

async fn drop_tempdb(dbname: String, base_config: &sqlx::postgres::PgConnectOptions) -> Result<(), sqlx::Error> {
	let mut conn = sqlx::PgConnection::connect_with(&base_config).await?;
	sqlx::raw_sql(&format!(r#"drop database if exists "{dbname}";"#))
		.execute(&mut conn).await?;
	Ok(())
}

async fn compute_diff(
	pgschema: &str,
	current: &sqlx::postgres::PgConnectOptions,
	intended: &sqlx::postgres::PgConnectOptions,
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
		.arg(convert_db_url(current))
		.arg(convert_db_url(intended))
		.output()
		.await
		.map_err(js_err)?;

	if !output.stderr.is_empty() {
		let e = format!("migra failed: {}\n\n{}", output.status, String::from_utf8_lossy(&output.stderr));
		return Err(deno_error::JsErrorBox::generic(e));
	}
	Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn convert_db_url(url: &sqlx::postgres::PgConnectOptions) -> String {
	use sqlx::ConnectOptions;
	String::from(url.to_url_lossy())
		.replace("postgres://", "postgresql://")
		.replace("&statement-cache-capacity=100", "")
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
	migrator_role_url: PgOpt,
	new_ruleset_id: sqlx::types::Uuid,
) -> Result<(), sqlx::Error> {
	let db_migration = sqlx::query!(
		r#"select m as "db_migration!" from votebase_catalog.apply_candidate($1) as t(m);"#,
		new_ruleset_id,
	).fetch_one(pool).await?.db_migration;

	let mut migrator_connection = sqlx::PgConnection::connect_with(&migrator_role_url).await?;
	sqlx::raw_sql(&db_migration).execute(&mut migrator_connection).await?;
	Ok(())
}

pub async fn create_ruleset(
	pool: &crate::PgPool,
	parent_full_path: Option<&str>, name: &str,
	action_names: &Vec<String>, view_names: &Vec<String>,
	ruleset_code: &str, db_schema: &str,
) -> Result<(), sqlx::Error> {
	let mut tx = pool.begin().await?;

	let ruleset = sqlx::query!(r#"
		insert into votebase_catalog.ruleset (
			parent_full_path, "name", actions, views, code, db_schema
		) values (
			$1, $2, $3, $4, $5, $6
		) returning full_path, migrator_pass, action_pass, view_pass;
	"#, parent_full_path, name, action_names, view_names, ruleset_code, db_schema)
		.fetch_one(&mut *tx).await?;

	sqlx::raw_sql(&format!(include_str!("./create-ruleset.sql"),
		full_path=ruleset.full_path,
		migrator_pass=ruleset.migrator_pass, action_pass=ruleset.action_pass, view_pass=ruleset.view_pass,
	)).execute(&mut *tx).await?;
	tx.commit().await?;

	let migrator_options = pool.connect_options().as_ref().clone()
		.username(&format!("role:{}|migrator", ruleset.full_path))
		.password(&ruleset.migrator_pass);

	let mut migrator_conn = sqlx::PgConnection::connect_with(&migrator_options).await?;
	sqlx::raw_sql(&db_schema).execute(&mut migrator_conn).await?;

	Ok(())
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
	fn_role_url: PgOpt,
	server_opt: PgOpt,
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
	runtime.set_fn_opt(fn_role_url);
	runtime.set_server_opt(server_opt);
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
