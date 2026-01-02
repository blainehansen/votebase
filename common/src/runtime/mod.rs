#[cfg(test)]
mod test;

mod scheduling;
pub use scheduling::{ScheduledActionQueue};

pub mod rulesets;
use rulesets::{op_propose_self_replacement};

mod members;
use members::{op_enroll_member, op_remove_member_by_email, op_remove_member_by_uuid};

mod sql;
use sql::{op_sql_fetch_all, op_sql_fetch_one, op_sql_fetch_optional, op_sql_execute_statement, op_sql_execute_statements};

mod dom;
use dom::{op_fetch};

use deno_core::{v8, OpState};
use log::info;
use crate::{postgres, FnRolePg, FnServerPg, FnType, PgClient, PgConfig, PgPool, RoleType, format_ruleset_role /*format_ruleset_schema*/};

// TODO RuntimeError should be narrowed to only the things that can go wrong during deno execution, and other broader errors should contain it

#[derive(thiserror::Error, Debug)]
pub enum RuntimeError {
	#[error(transparent)]
	SerdeV8(#[from] deno_core::serde_v8::Error),
	#[error(transparent)]
	DenoCoreError(#[from] deno_core::error::CoreError),
	#[error(transparent)]
	ModuleResolutionError(#[from] deno_core::ModuleResolutionError),
	#[error(transparent)]
	StdIoError(#[from] std::io::Error),

	#[error(transparent)]
	PostgresError(#[from] postgres::Error),
	#[error(transparent)]
	PoolError(#[from] crate::deadpool::PoolError),
	#[error(transparent)]
	UuidParseError(#[from] uuid::Error),
	#[error(transparent)]
	TranspileError(#[from] transpile_utils::TranspileError),

	#[error("internal error: {0}")]
	OtherError(String),
}

impl Into<deno_error::JsErrorBox> for RuntimeError {
	fn into(self) -> deno_error::JsErrorBox {
		match self {
			RuntimeError::SerdeV8(e) => deno_error::JsErrorBox::from_err(e),
			RuntimeError::DenoCoreError(e) => deno_error::JsErrorBox::from_err(e),
			RuntimeError::ModuleResolutionError(e) => deno_error::JsErrorBox::from_err(e),
			RuntimeError::StdIoError(e) => deno_error::JsErrorBox::from_err(e),

			RuntimeError::PostgresError(e) => deno_error::JsErrorBox::generic(e.to_string()),
			RuntimeError::PoolError(e) => deno_error::JsErrorBox::generic(e.to_string()),
			RuntimeError::UuidParseError(e) => deno_error::JsErrorBox::generic(e.to_string()),
			RuntimeError::TranspileError(e) => deno_error::JsErrorBox::generic(e.to_string()),

			RuntimeError::OtherError(m) => deno_error::JsErrorBox::generic(m),
		}
	}
}

pub fn js_err<E: std::error::Error>(e: E) -> deno_error::JsErrorBox {
	deno_error::JsErrorBox::generic(e.to_string())
}

pub fn run_err<E: Into<RuntimeError>>(e: E) -> deno_error::JsErrorBox {
	e.into().into()
}

deno_core::extension!(
	votebase,
	ops = [
		op_register_action,
		op_register_view,
		// op_create_recurring_action,
		// op_remove_recurring_action,
		// op_schedule_action,
		// op_unschedule_action,
		op_enroll_member,
		op_remove_member_by_email,
		op_remove_member_by_uuid,
		op_propose_self_replacement,
		// op_create_child_ruleset,
		// op_delete_child_ruleset,
		// op_propose_child_replacement,
		// op_replace_child,
		op_sql_fetch_all,
		op_sql_fetch_one,
		op_sql_fetch_optional,
		op_sql_execute_statement,
		op_sql_execute_statements,
		// op_set_timeout,
		op_fetch,
	],
);

static RUNTIME_SNAPSHOT: &[u8] =
	include_bytes!(concat!(env!("OUT_DIR"), "/VOTEBASE_SNAPSHOT.bin"));

const MAIN_SPECIFIER: &'static str = "votebase:<main>";


struct RunInfo {
	current_ruleset_path: String,
}

fn demand_external_allowed(state: &OpState) -> Result<(), deno_error::JsErrorBox> {
	let external_allowed = state.borrow::<bool>();
	if !external_allowed {
		return Err(deno_error::JsErrorBox::generic(ERR_EXTERNAL_NOT_ALLOWED))
	}
	Ok(())
}


const ERR_EXTERNAL_NOT_ALLOWED: &'static str = "runtime functions that interact with timers or the outside world (such as database or http operations) aren't allowed outside of an action or view";

pub type VotebaseFnMap = std::collections::HashMap<String, VotebaseFn>;

#[derive(Debug)]
pub enum VotebaseFn {
	Action(v8::Global<v8::Function>),
	View(v8::Global<v8::Function>),
}

fn register_fn(
	state: &mut OpState,
	fn_name: &str,
	// #[serde] schema: serde_json::Value,
	is_action: bool,
	func: v8::Global<v8::Function>,
) -> Result<(), deno_error::JsErrorBox> {
	// let validator = jsonschema::draft7::new(schema)?;

	// if the js side calls zodToJsonSchema and then we use a serde serializer to decode one of these:
	// https://docs.rs/jsonschema/latest/jsonschema/struct.Validator.html
	// then we've effectively demanded actions/views to type their inputs!
	let func = match is_action {
		true => VotebaseFn::Action(func),
		false => VotebaseFn::View(func),
	};

	let fn_map = state.borrow_mut::<VotebaseFnMap>();
	use std::collections::hash_map::Entry;
	match fn_map.entry(fn_name.to_owned()) {
		Entry::Vacant(entry) => {
			entry.insert(func);
			Ok(())
		},
		Entry::Occupied(_) => {
			Err(deno_error::JsErrorBox::generic(format!("already a View or Action with name {fn_name}")))
		}
	}
}

#[deno_core::op2]
fn op_register_action(
	state: &mut OpState,
	#[string] fn_name: &str,
	// #[serde] schema: serde_json::Value,
	#[global] func: v8::Global<v8::Function>,
) -> Result<(), deno_error::JsErrorBox> {
	register_fn(state, fn_name, true, func)
}

#[deno_core::op2]
fn op_register_view(
	state: &mut OpState,
	#[string] fn_name: &str,
	// #[serde] schema: serde_json::Value,
	#[global] func: v8::Global<v8::Function>,
) -> Result<(), deno_error::JsErrorBox> {
	register_fn(state, fn_name, false, func)
}

pub struct Runtime {
	js_runtime: deno_core::JsRuntime,
}

impl Runtime {
	pub async fn new(code: &str) -> Result<Self, RuntimeError> {
		let js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
			module_loader: None,
			startup_snapshot: Some(RUNTIME_SNAPSHOT),
			extensions: vec![votebase::init()],
			..Default::default()
		});

		let mut runtime = Self { js_runtime };
		runtime.set_fn_map();
		runtime.set_external_allowed(false);

		let (code, _) = transpile_utils::transpile_typescript(
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
		self.js_runtime.op_state().borrow_mut().put(VotebaseFnMap::new());
	}
	pub fn take_fn_map(&mut self) -> VotebaseFnMap {
		self.js_runtime.op_state().borrow_mut().take()
	}

	pub fn set_external_allowed(&mut self, allowed: bool) {
		self.js_runtime.op_state().borrow_mut().put(allowed);
	}

	pub fn set_run_info(&mut self, run_info: RunInfo) {
		self.js_runtime.op_state().borrow_mut().put(run_info);
	}
	pub fn set_action_queue(&mut self, spawner: ScheduledActionQueue) {
		self.js_runtime.op_state().borrow_mut().put(spawner);
	}
	pub fn set_fn_server_pg(&mut self, opt: FnServerPg) {
		self.js_runtime.op_state().borrow_mut().put(opt);
	}
	pub fn set_fn_role_pg(&mut self, client: FnRolePg) {
		self.js_runtime.op_state().borrow_mut().put(client);
	}
}

pub async fn run_action(
	current_full_path: String,
	ruleset_code: &str,
	action_name: &str,
	action_pass: &str,
	migrator_pass: &str,
	arg: serde_json::Value,
	base_config: PgConfig,
	server_role_client: PgClient,
	scheduled_action_queue: ScheduledActionQueue,
) -> Result<(), RuntimeError> {
	let migrator_pg = FnRolePg::for_role(&current_full_path, &base_config, RoleType::Migrator, migrator_pass);
	let action_pg = FnRolePg::for_role(&current_full_path, &base_config, RoleType::Action, action_pass);

	let new_ruleset_id = run_function::<Option<String>>(
		current_full_path, ruleset_code, action_name, arg, FnType::Action,
		action_pg, server_role_config, server_role_client, scheduled_action_queue,
	).await?;

	if let Some(new_ruleset_id) = new_ruleset_id {
		let new_ruleset_id = new_ruleset_id.parse::<uuid::Uuid>()?;
		info!("apply_candidate {new_ruleset_id}");
		rulesets::replace_ruleset(&server_role_client, &migrator_pg, &new_ruleset_id).await?;
	}

	Ok(())
}

pub async fn run_view(
	current_full_path: String,
	ruleset_code: &str,
	view_name: &str,
	view_pass: &str,
	query: serde_json::Value,
	base_config: PgConfig,
	server_role_pool: PgPool,
	scheduled_action_queue: ScheduledActionQueue,
) -> Result<String, RuntimeError> {
	let view_role_config = FnRolePg::for_role(&current_full_path, &base_config, RoleType::View, view_pass);

	run_function(
		ruleset_code, view_name, query, FnType::View,
		view_role_config, base_config, scheduled_action_queue,
	).await
}

async fn run_function<'r, V: deno_core::serde::Deserialize<'r>>(
	ruleset_code: &str,
	function_name: &str,
	function_arg: serde_json::Value,
	function_type: FnType,
	server_role_pool: PgPool,
	fn_role_pg: FnRolePg,
	scheduled_action_queue: ScheduledActionQueue,
) -> Result<V, RuntimeError> {
	let mut runtime = Runtime::new(ruleset_code).await.map_err(|e| RuntimeError::OtherError(e.to_string()))?;

	// fn_role_config encodes the user, and therefore the role and powers of the connection
	let fn_map = runtime.take_fn_map();
	let function = fn_map.get(function_name)
		.ok_or_else(|| RuntimeError::OtherError(format!("{} '{}' not found", function_type, function_name)))?;

	let function = match (function_type, function) {
		(FnType::Action, VotebaseFn::Action(function)) => function,
		(FnType::View, VotebaseFn::View(function)) => function,
		_ => { return Err(RuntimeError::OtherError(format!("'{}' isn't of type {}", function_name, function_type))) },
	};

	runtime.set_external_allowed(true);
	runtime.set_run_info(RunInfo { current_ruleset_path });
	runtime.set_action_queue(scheduled_action_queue);
	runtime.set_fn_server_pg(server_role_pool.into());
	runtime.set_fn_role_pg(fn_role_pg);

	// https://questions.deno.com/m/1201661871959310346
	// https://github.com/denoland/deno_core/issues/515
	// maybe a better way to do this that isn't based on registration? accessing things as a Local<Object>?
	// https://github.com/tailcallhq/tailcall/pull/1144/files#diff-2a08de1a47319583d4bb5559a203d98bb29ba13b2ea658cae1597231a6177770R40

	let function_arg = {
		deno_core::scope!(scope, runtime.js_runtime);
		let function_arg = deno_core::serde_v8::to_v8(scope, function_arg)?;
		v8::Global::new(scope, function_arg)
	};

	// TODO also pass user_id here, maybe with some other context in the future

	let call = runtime.js_runtime.call_with_args(function, &[function_arg]);
	let call_return_value = runtime.js_runtime
		.with_event_loop_promise(call, deno_core::PollEventLoopOptions::default())
		.await?;

	deno_core::scope!(scope, runtime.js_runtime);
	let call_return_value = v8::Local::new(scope, call_return_value);
	Ok(deno_core::serde_v8::from_v8(scope, call_return_value)?)
}

