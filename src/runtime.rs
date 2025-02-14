use std::{cell::RefCell, rc::Rc};
use deno_core::{v8, OpState};
use sqlx::Connection;

pub type DenoError = deno_core::error::AnyError;

pub fn js_err<E: std::error::Error>(e: E) -> deno_error::JsErrorBox {
	deno_error::JsErrorBox::generic(e.to_string())
}

#[deno_core::op2(async)]
#[string]
async fn op_fetch(
	state: Rc<RefCell<OpState>>,
	#[string] url: String,
) -> Result<String, deno_error::JsErrorBox> {
	let state = state.as_ref().borrow();
	let external_allowed = deno_core::_ops::opstate_borrow::<bool>(&state);
	if !external_allowed {
		return Err(deno_error::JsErrorBox::generic(ERR_EXTERNAL_NOT_ALLOWED))
	}

	let body = reqwest::get(url).await.map_err(js_err)?.text().await.map_err(js_err)?;
	Ok(body)
}

#[deno_core::op2(async)]
async fn op_set_timeout(
	state: Rc<RefCell<OpState>>,
	delay: f64
) -> Result<(), deno_error::JsErrorBox> {
	let state = state.as_ref().borrow();
	let external_allowed = deno_core::_ops::opstate_borrow::<bool>(&state);
	if !external_allowed {
		return Err(deno_error::JsErrorBox::generic(ERR_EXTERNAL_NOT_ALLOWED))
	}

	tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
	Ok(())
}


const ERR_EXTERNAL_NOT_ALLOWED: &'static str = "runtime functions that interact with timers or the outside world (such as database or http operations) aren't allowed outside the context of an action or view";

#[deno_core::op2(async)]
async fn op_sql_execute_many(
	state: Rc<RefCell<OpState>>,
	#[string] sql: String,
) -> Result<u32, deno_error::JsErrorBox> {
	let mut state = state.as_ref().borrow_mut();
	let external_allowed = deno_core::_ops::opstate_borrow::<bool>(&state);
	if !external_allowed {
		return Err(deno_error::JsErrorBox::generic(ERR_EXTERNAL_NOT_ALLOWED))
	}

	let connection = deno_core::_ops::opstate_borrow_mut::<sqlx::PgConnection>(std::ops::DerefMut::deref_mut(&mut state));
	let result = sqlx::raw_sql(&sql).execute(connection).await.map_err(js_err)?;
	Ok(result.rows_affected().try_into().map_err(js_err)?)
}

// https://github.com/denoland/deno_core/issues/515
// https://discord.com/channels/684898665143206084/1022163295895027722/threads/1201661871959310346
// https://gist.github.com/alshdavid/c9e5bc0d794e3ec9dba6afaa689b704e#file-main-rs-L51

// https://discord.com/channels/684898665143206084/1022163295895027722/threads/1074150763460313128

type FnMap = std::collections::HashMap<String, Fn>;

#[derive(Debug)]
enum Fn {
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
	let state = state.as_ref().borrow();
	let external_allowed = deno_core::_ops::opstate_borrow::<bool>(&state);
	if !external_allowed {
		return Err(deno_error::JsErrorBox::generic(ERR_EXTERNAL_NOT_ALLOWED))
	}

	// TODO need to get these populated
	let server_pool = deno_core::_ops::opstate_borrow::<crate::PgPool>(&state);
	let current_full_path = deno_core::_ops::opstate_borrow::<String>(&state);

	let mut js_runtime = create_js_runtime();
	set_external_allowed(&mut js_runtime, false);
	load_code(&candidate.code, &mut js_runtime).await.map_err(|e| js_err(e.root_cause()))?;

	let inner_state = js_runtime.op_state();
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

	let candidate_uuid = sqlx::query!(
		r#"select u as "u!" from votebase_catalog.insert_candidate_replacement($1, $2, $3, $4, $5, $6) as u(u);"#,
		&current_full_path, &dbg!(actions), &dbg!(views), &candidate.code, &candidate.db_schema, &candidate.db_migration,
	).fetch_one(server_pool).await.map_err(js_err)?;

	Ok(candidate_uuid.u.into())
}

pub async fn replace_ruleset(
	pool: &crate::PgPool,
	new_ruleset_id: sqlx::types::Uuid,
) -> Result<(), sqlx::Error> {
	// TODO also need code to migrate to new ruleset
	// ought to do *all* of this in a transaction? that's not even possible with ddl statements is it?
	sqlx::query!("call votebase_catalog.apply_candidate($1);", new_ruleset_id)
		.execute(pool).await?;

	Ok(())
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
	state = |state: &mut deno_core::OpState| {
		// fn_map
		state.put(FnMap::new());
		// external_allowed
		state.put(false);
		// current_full_path
		// TODO?
	},
);

static RUNTIME_SNAPSHOT: &[u8] =
	include_bytes!(concat!(env!("OUT_DIR"), "/VOTEBASE_SNAPSHOT.bin"));

pub fn create_js_runtime() -> deno_core::JsRuntime {
	deno_core::JsRuntime::new(deno_core::RuntimeOptions {
		module_loader: None,
		startup_snapshot: Some(RUNTIME_SNAPSHOT),
		extensions: vec![votebase::init_ops()],
		..Default::default()
	})
}
async fn set_pg_connection(
	js_runtime: &mut deno_core::JsRuntime,
	connect_options: &sqlx::postgres::PgConnectOptions,
) -> sqlx::Result<()> {
	let connection: sqlx::PgConnection = sqlx::PgConnection::connect_with(connect_options).await?;
	js_runtime.op_state().borrow_mut().put(connection);
	Ok(())
}
fn set_external_allowed(js_runtime: &mut deno_core::JsRuntime, external_allowed: bool) {
	js_runtime.op_state().borrow_mut().put(external_allowed);
}
const MAIN_SPECIFIER: &'static str = "votebase:<main>";

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

async fn load_code(
	ruleset_code: &str,
	js_runtime: &mut deno_core::JsRuntime,
) -> Result<(), DenoError> {
	let (ruleset_code, _) = transpile_helpers::transpile_typescript(
		deno_core::ascii_str!(MAIN_SPECIFIER).into(),
		ruleset_code.to_string().into(),
	)?;
	let specifier = deno_core::resolve_url(MAIN_SPECIFIER)?;
	let mod_id = js_runtime.load_main_es_module_from_code(&specifier, ruleset_code).await?;
	let result = js_runtime.mod_evaluate(mod_id);
	js_runtime.run_event_loop(Default::default()).await?;
	Ok(result.await?)
}

pub async fn run_function<'r, V: deno_core::serde::Deserialize<'r>>(
	ruleset_code: String,
	function_name: &str,
	function_arg: serde_json::Value,
	function_type: FnType,
	db_url: &sqlx::postgres::PgConnectOptions,
) -> Result<V, DenoError> {
	// TODO set current_ruleset_id

	let mut js_runtime = create_js_runtime();
	// db_url encodes the user, and therefore the role and powers of the connection
	set_external_allowed(&mut js_runtime, false);
	load_code(&ruleset_code, &mut js_runtime).await?;

	let fn_map: FnMap = js_runtime.op_state().borrow_mut().take();
	let function = fn_map.get(function_name)
		.ok_or_else(|| deno_core::anyhow::anyhow!("{} '{}' not found", function_type, function_name))?;

	let function = match (function_type, function) {
		(FnType::Action, Fn::Action(function)) => function,
		(FnType::View, Fn::View(function)) => function,
		_ => { return Err(deno_core::anyhow::anyhow!("'{}' isn't a {}", function_name, function_type)) },
	};

	let function_arg = {
		let mut scope = js_runtime.handle_scope();
		let function_arg = deno_core::serde_v8::to_v8(&mut scope, function_arg)?;
		v8::Global::new(&mut scope, function_arg)
	};

	set_external_allowed(&mut js_runtime, true);
	set_pg_connection(&mut js_runtime, db_url).await?;
	let call = js_runtime.call_with_args(&function, &[function_arg]);
	let call_return_value = js_runtime
		.with_event_loop_promise(call, deno_core::PollEventLoopOptions::default())
		.await?;

	let mut scope = js_runtime.handle_scope();
	let call_return_value = v8::Local::new(&mut scope, call_return_value);
	Ok(deno_core::serde_v8::from_v8(&mut scope, call_return_value)?)
}


#[cfg(test)]
mod tests {
	use super::*;

	const DEV_DB_URL: &'static str = "postgres://dev_user:dev_password@localhost:5432/dev_db";

	#[tokio::test]
	async fn test_propose_self_replacement() {
		let mut js_runtime = create_js_runtime();
		set_external_allowed(&mut js_runtime, true);

		load_code(
			r#"
				const u = Deno.core.ops.op_propose_self_replacement({
					code: `
						votebase.registerAction("action1", () => {});
						votebase.registerAction("action2", () => {});
						votebase.registerView("view1", () => {});
					`,
					db_schema: "", db_migration: "",
				})
				console.log(u)
			"#.into(),
			&mut js_runtime,
		).await.unwrap();

		// let state = js_runtime.op_state();
		// let state = state.as_ref().borrow();
		// let fn_map = deno_core::_ops::opstate_borrow::<FnMap>(&state);
	}

	#[tokio::test]
	async fn can_do_sql() {
		let mut js_runtime = create_js_runtime();
		let db_url = DEV_DB_URL.parse().unwrap();
		set_pg_connection(&mut js_runtime, &db_url).await.unwrap();
		set_external_allowed(&mut js_runtime, true);

		let script = r#"
			const result = await Deno.core.ops.op_sql_execute_many("select 1 as hey")
			Deno.core.print(result)
		"#;
		let specifier = deno_core::resolve_url(MAIN_SPECIFIER).unwrap();
		let mod_id = js_runtime.load_main_es_module_from_code(&specifier, script).await.unwrap();
		let result = js_runtime.mod_evaluate(mod_id);
		js_runtime.run_event_loop(Default::default()).await.unwrap();
		result.await.unwrap();
	}

	#[tokio::test]
	async fn run_function_basics() {
		let db_url = DEV_DB_URL.parse().unwrap();
		let result = run_function::<serde_json::Value>(
			r#"
				await Deno.core.ops.op_sql_execute_many("select 1")
				votebase.registerAction("test_action", async () => {
					return true
				})
			"#.to_string(),
			"test_action", serde_json::json!(null), FnType::Action, &db_url,
		).await.unwrap_err();
		assert!(result.to_string().contains(ERR_EXTERNAL_NOT_ALLOWED));

		let result = run_function::<serde_json::Value>(
			r#"
				votebase.registerAction("test_action", async () => {
					return await Deno.core.ops.op_sql_execute_many("select 1")
				})
			"#.to_string(),
			"test_action", serde_json::json!(null), FnType::Action, &db_url,
		).await.unwrap();
		assert_eq!(result, serde_json::json!(1));

		let result = run_function::<serde_json::Value>(
			r#"
				await Deno.core.ops.op_sql_execute_many("select 1")
				votebase.registerView("test_view", async () => {
					return true
				})
			"#.to_string(),
			"test_view", serde_json::json!(null), FnType::View, &db_url,
		).await.unwrap_err();
		assert!(result.to_string().contains(ERR_EXTERNAL_NOT_ALLOWED));

		let result = run_function::<serde_json::Value>(
			r#"
				votebase.registerView("test_view", async () => {
					return await Deno.core.ops.op_sql_execute_many("select 1")
				})
			"#.to_string(),
			"test_view", serde_json::json!(null), FnType::View, &db_url,
		).await.unwrap();
		assert_eq!(result, serde_json::json!(1));
	}
}
