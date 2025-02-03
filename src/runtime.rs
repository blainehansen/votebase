use std::{cell::RefCell, rc::Rc};
use deno_core::{v8};
use sqlx::Connection;

pub type DenoError = deno_core::error::AnyError;

pub fn js_err<E: std::error::Error>(e: E) -> deno_error::JsErrorBox {
	deno_error::JsErrorBox::generic(e.to_string())
}

#[deno_core::op2(async)]
#[string]
async fn op_fetch(#[string] url: String) -> Result<String, deno_error::JsErrorBox> {
	let body = reqwest::get(url).await.map_err(js_err)?.text().await.map_err(js_err)?;
	Ok(body)
}

#[deno_core::op2(async)]
async fn op_set_timeout(delay: f64) -> Result<(), deno_error::JsErrorBox> {
	tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
	Ok(())
}

use deno_core::OpState;

const ERR_EXTERNAL_NOT_ALLOWED: &'static str = "runtime functions that interact with the outside world (such as database or http operations) aren't allowed outside the context of an action or view";

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


static RUNTIME_SNAPSHOT: &[u8] =
	include_bytes!(concat!(env!("OUT_DIR"), "/VOTEBASE_SNAPSHOT.bin"));

type FunctionMap = std::collections::HashMap<String, v8::Global<v8::Function>>;

struct ActionMap(FunctionMap);
struct ViewMap(FunctionMap);

// https://github.com/denoland/deno_core/issues/515
// https://discord.com/channels/684898665143206084/1022163295895027722/threads/1201661871959310346
// https://gist.github.com/alshdavid/c9e5bc0d794e3ec9dba6afaa689b704e#file-main-rs-L51

// https://discord.com/channels/684898665143206084/1022163295895027722/threads/1074150763460313128

#[deno_core::op2]
fn op_register_action(
	#[state] function_state: &mut ActionMap,
	#[string] key: String,
	#[global] func: v8::Global<v8::Function>,
) {
	function_state.0.insert(key, func);
}
#[deno_core::op2]
fn op_register_view(
	#[state] function_state: &mut ViewMap,
	#[string] key: String,
	#[global] func: v8::Global<v8::Function>,
) {
	function_state.0.insert(key, func);
}

deno_core::extension!(
	votebase,
	ops = [
		op_fetch,
		op_set_timeout,
		op_register_action,
		op_register_view,
		op_sql_execute_many,
	],
	state = |state: &mut deno_core::OpState| {
		state.put(ActionMap(std::collections::HashMap::<String, v8::Global<v8::Function>>::new()));
		state.put(ViewMap(std::collections::HashMap::<String, v8::Global<v8::Function>>::new()));
	},
);

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

pub async fn run_function<'r, V: deno_core::serde::Deserialize<'r>>(
	ruleset_code: String,
	function_name: &str,
	function_arg: serde_json::Value,
	function_type: FnType,
	db_url: &sqlx::postgres::PgConnectOptions,
) -> Result<V, DenoError> {
	let mut js_runtime = create_js_runtime();
	// db_url encodes the user, and therefore the role and powers of the connection
	set_pg_connection(&mut js_runtime, db_url).await?;
	set_external_allowed(&mut js_runtime, false);

	let (ruleset_code, _) = transpile_helpers::transpile_typescript(
		deno_core::ascii_str!(MAIN_SPECIFIER).into(),
		ruleset_code.into(),
	)?;
	let specifier = deno_core::resolve_url(MAIN_SPECIFIER)?;
	let mod_id = js_runtime.load_main_es_module_from_code(&specifier, ruleset_code).await?;
	let result = js_runtime.mod_evaluate(mod_id);
	js_runtime.run_event_loop(Default::default()).await?;
	result.await?;

	let (function_map, function_type) = match function_type {
		FnType::Action => { let map: ActionMap = js_runtime.op_state().borrow_mut().take(); (map.0, "action") },
		FnType::View => { let map: ViewMap = js_runtime.op_state().borrow_mut().take(); (map.0, "view") },
	};

	let function = function_map.get(function_name)
		.ok_or_else(|| deno_core::anyhow::anyhow!("{} '{}' not found", function_type, function_name))?;

	let function_arg = {
		let mut scope = js_runtime.handle_scope();
		let function_arg = deno_core::serde_v8::to_v8(&mut scope, function_arg)?;
		v8::Global::new(&mut scope, function_arg)
	};

	set_external_allowed(&mut js_runtime, true);
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
