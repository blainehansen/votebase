#[cfg(test)]
mod test;
mod rulesets;

use std::{cell::RefCell, collections::HashMap, rc::Rc};
use deno_core::{v8, OpState};
use uuid::Uuid;
use crate::{queries, PgConfig, PgPool, PgClient, FnType, RoleType, ScheduledActionKind, format_ruleset_schema, format_ruleset_role, postgres};

#[derive(thiserror::Error, Debug)]
pub enum RuntimeError {
	#[error(transparent)]
	SerdeV8(#[from] deno_core::serde_v8::Error),
	#[error(transparent)]
	DenoCoreError(#[from] deno_core::error::CoreError),
	#[error(transparent)]
	PostgresError(#[from] postgres::Error),
	#[error(transparent)]
	PoolError(#[from] crate::deadpool::PoolError),
	#[error(transparent)]
	UuidParseError(#[from] uuid::Error),
	#[error(transparent)]
	TranspileError(#[from] votebase_transpile::TranspileError),
	#[error(transparent)]
	ModuleResolutionError(#[from] deno_core::ModuleResolutionError),

	#[error("internal error: {0}")]
	OtherError(String),
}

impl Into<deno_error::JsErrorBox> for RuntimeError {
	fn into(self) -> deno_error::JsErrorBox {
		match self {
			RuntimeError::SerdeV8(e) => deno_error::JsErrorBox::from_err(e),
			RuntimeError::DenoCoreError(e) => deno_error::JsErrorBox::from_err(e),
			RuntimeError::PostgresError(e) => deno_error::JsErrorBox::generic(e.to_string()),
			RuntimeError::PoolError(e) => deno_error::JsErrorBox::generic(e.to_string()),
			RuntimeError::UuidParseError(e) => deno_error::JsErrorBox::generic(e.to_string()),
			RuntimeError::TranspileError(e) => deno_error::JsErrorBox::generic(e.to_string()),
			RuntimeError::ModuleResolutionError(e) => deno_error::JsErrorBox::from_err(e),
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

pub struct Runtime {
	js_runtime: deno_core::JsRuntime,
}

impl Runtime {
	pub async fn new(code: &str) -> Result<Self, RuntimeError> {
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

	pub fn set_action_queue(&mut self, spawner: ScheduledActionQueue) {
		self.js_runtime.op_state().borrow_mut().put(spawner);
	}
	pub fn set_server_pool(&mut self, pool: PgPool) {
		self.js_runtime.op_state().borrow_mut().put(pool);
	}
	pub fn set_server_config(&mut self, opt: PgConfig) {
		self.js_runtime.op_state().borrow_mut().put(ServerPgConfig(opt));
	}
	pub fn set_fn_config(&mut self, config: PgConfig) {
		self.js_runtime.op_state().borrow_mut().put(config);
	}

	pub fn set_current_full_path(&mut self, path: String) {
		self.js_runtime.op_state().borrow_mut().put(path);
	}
}

deno_core::extension!(
	votebase,
	ops = [
		op_fetch,
		// op_set_timeout,
		op_sql_fetch_all,
		op_sql_fetch_one,
		op_sql_fetch_optional,
		op_sql_execute_statement,
		op_sql_execute_statements,

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
struct ServerPgConfig(PgConfig);

fn demand_external_allowed(state: &RefCell<OpState>) -> Result<(), deno_error::JsErrorBox> {
	let state = state.borrow();
	let external_allowed = deno_core::_ops::opstate_borrow::<bool>(&state);
	if !external_allowed {
		return Err(deno_error::JsErrorBox::generic(ERR_EXTERNAL_NOT_ALLOWED))
	}
	Ok(())
}

// op_fetch: (url: string) => Promise<string>,
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

#[derive(serde::Deserialize, Debug)]
#[serde(variant_identifier)]
enum PgTypeHint {
	Json, Bool, Text, Bytea, Hstore,
	I16, I32, I64, F32, F64,
	// | `u32`: OID
}

fn prepare_param(
	raw_param: serde_json::Value,
	hint: PgTypeHint,
) -> Result<(Box<(dyn postgres::types::ToSql + Sync)>, postgres::types::Type), RuntimeError> {
	#[allow(non_snake_case)]
	let HSTORE = postgres::types::Type::new("hstore".to_string(), 0, postgres::types::Kind::Simple, "public".to_string());
	use postgres::types::Type;
	use serde_json::Value as V;

	Ok(match (hint, raw_param) {
		(PgTypeHint::Json, V::Null) => (Box::new(None::<V>), Type::JSON),
		(PgTypeHint::Json, raw_param) => (Box::new(raw_param), Type::JSON),

		// (PgTypeHint::JsonArray, V::Null) => (Box::new(None::<V>), Type::JSON_ARRAY),
		// (PgTypeHint::JsonArray, V::Array(a)) => (Box::new(a), Type::JSON_ARRAY),

		(PgTypeHint::Bool, V::Null) => (Box::new(None::<bool>), Type::BOOL),
		(PgTypeHint::Bool, V::Bool(b)) => (Box::new(b), Type::BOOL),

		(PgTypeHint::Text, V::Null) => (Box::new(None::<String>), Type::TEXT),
		(PgTypeHint::Text, V::String(s)) => (Box::new(s), Type::TEXT),

		(PgTypeHint::Bytea, V::Null) => (Box::new(None::<Vec<u8>>), Type::BYTEA),
		(PgTypeHint::Bytea, V::Array(n)) => {
			let v: Vec<_> = n.into_iter().map(|n| {
				let n = n.as_u64().ok_or_else(|| RuntimeError::OtherError("number wasn't byte".to_string()))?;
				u8::try_from(n).map_err(|e| RuntimeError::OtherError(e.to_string()))
			}).collect::<Result<_, _>>()?;

			(Box::new(v), Type::BYTEA)
		},

		(PgTypeHint::Hstore, V::Null) => (Box::new(None::<HashMap<String, Option<String>>>), HSTORE),
		(PgTypeHint::Hstore, V::Object(m)) => {
			let m = m.into_iter().map(|(k, v)| {
				let v = match v {
					V::Null => None,
					V::String(s) => Some(s),
					_ => { return Err(RuntimeError::OtherError("expected string".to_string())) },
				};

				Ok((k, v))
			}).collect::<Result<HashMap<_, _>, _>>()?;
			(Box::new(m), HSTORE)
		},

		(PgTypeHint::I16 | PgTypeHint::I32 | PgTypeHint::I64 | PgTypeHint::F32 | PgTypeHint::F64, V::Null) =>
			(Box::new(None::<f64>), Type::FLOAT8),
		(PgTypeHint::I16 | PgTypeHint::I32 | PgTypeHint::I64 | PgTypeHint::F32 | PgTypeHint::F64, V::Number(n)) =>
			if n.is_i64() {
				(Box::new(n.as_i64().unwrap()), Type::INT8)
			} else if n.is_u64() {
				return Err(RuntimeError::OtherError("couldn't represent number".to_string()));
			} else {
				(Box::new(n.as_f64().ok_or_else(|| RuntimeError::OtherError("couldn't represent number".to_string()))?), Type::FLOAT8)
			},

		(hint, raw_param) => {
			return Err(RuntimeError::OtherError(format!("mismatched param and hint: {:?}, {:?}", raw_param, hint)));
		}
	})
}

fn prepare_params(
	raw_params: Vec<serde_json::Value>,
	hints: Vec<PgTypeHint>,
) -> Result<Vec<(Box<(dyn postgres::types::ToSql + Sync)>, postgres::types::Type)>, RuntimeError> {
	if raw_params.len() != hints.len() {
		return Err(RuntimeError::OtherError("params and hints must be the same length".to_string()))
	}

	raw_params.into_iter().zip(hints.into_iter())
		.map(|(raw_param, hint)| prepare_param(raw_param, hint)).collect()
}


#[derive(serde::Deserialize, Debug)]
#[serde(untagged)]
enum RetHint {
	Scalar(PgTypeHint),
	Columns(Vec<(String, PgTypeHint)>),
}

fn convert_row(row: postgres::Row, ret: &RetHint) -> Result<serde_json::Value, RuntimeError> {
	let columns = row.columns();
	match ret {
		RetHint::Scalar(ret) => {
			if columns.len() != 1 {
				return Err(RuntimeError::OtherError("row doesn't have exactly 1 column".to_string()))
			}
			Ok(convert_col(&row, ret, 0)?)
		},
		RetHint::Columns(rets) => {
			if rets.len() != columns.len() {
				return Err(RuntimeError::OtherError("hints don't match columns".to_string()))
			}
			Ok(serde_json::Value::Object(rets.iter().zip(columns).enumerate().map(|(index, ((ret_name, ret), column))| {
				// TODO can we rely on the ordering of rets and columns?
				debug_assert!(ret_name == column.name());
				let name = ret_name.clone();
				let value = convert_col(&row, ret, index)?;
				Ok((name, value))
			}).collect::<Result<_, RuntimeError>>()?))
		},
	}
}

// const DATETIME_FORMAT: chrono::format::strftime::StrftimeItems = chrono::format::strftime::StrftimeItems::new("%+");
// const DATE_FORMAT: chrono::format::strftime::StrftimeItems = chrono::format::strftime::StrftimeItems::new("%F");

#[inline]
fn convert_col(row: &postgres::Row, ret: &PgTypeHint, i: usize) -> Result<serde_json::Value, postgres::Error> {
	Ok(match ret {
		PgTypeHint::Json => row.try_get::<_, serde_json::Value>(i)?,
		PgTypeHint::Bool => row.try_get::<_, bool>(i)?.into(),
		PgTypeHint::Text => row.try_get::<_, String>(i)?.into(),
		PgTypeHint::Bytea => row.try_get::<_, Vec<u8>>(i)?.into(),
		PgTypeHint::Hstore => row.try_get::<_, HashMap<String, Option<String>>>(i)?.into_iter()
			.map(|(k, v)| (k, v.into())).collect::<serde_json::Map<_, _>>().into(),
		PgTypeHint::I16 => row.try_get::<_, i16>(i)?.into(),
		PgTypeHint::I32 => row.try_get::<_, i32>(i)?.into(),
		PgTypeHint::I64 => row.try_get::<_, i64>(i)?.into(),
		PgTypeHint::F32 => row.try_get::<_, f32>(i)?.into(),
		PgTypeHint::F64 => row.try_get::<_, f64>(i)?.into(),
	})
}

// #[deno_core::op2]
// fn ahha<'s>(scope: &'s mut v8::HandleScope) -> v8::Local<'s, v8::Value> {
// 	v8::String::new(scope, "wassup").unwrap().into()
// }

// op_sql_fetch_all: <P extends ParamHint[], R extends FullRetHint>
// 	(sql: string, params: ActualParams<P>, hints: P, ret: R) => Promise<ActualRet<R>[]>,
#[deno_core::op2(async)]
#[serde]
async fn op_sql_fetch_all(
	state: Rc<RefCell<OpState>>,
	#[string] sql: String,
	#[serde] raw_params: Vec<serde_json::Value>,
	#[serde] hints: Vec<PgTypeHint>,
	#[serde] ret: RetHint,
) -> Result<Vec<serde_json::Value>, deno_error::JsErrorBox> {
	let state = state.as_ref();
	demand_external_allowed(state)?;
	let state = state.borrow();
	// TODO we're connecting every time here, which seems necessary for security, but terrible for performance
	let fn_config = deno_core::_ops::opstate_borrow::<PgConfig>(&state);
	let (client, connection) = fn_config.connect(postgres::NoTls).await.map_err(run_err)?;
	tokio::spawn(async move { if let Err(e) = connection.await { log::error!("DB connection error: {}", e); } });

	let params = prepare_params(raw_params, hints).map_err(run_err)?;
	let row_stream = client.query_typed_raw(&sql, params).await.map_err(run_err)?;
	use futures_util::{pin_mut, TryStreamExt};
	pin_mut!(row_stream);
	let rows = row_stream.try_collect::<Vec<_>>().await.map_err(run_err)?
		.into_iter()
		.map(|row| convert_row(row, &ret))
		.collect::<Result<Vec<_>, _>>().map_err(run_err)?;

	Ok(rows)
}

// op_sql_fetch_one: <P extends ParamHint[], R extends FullRetHint>
// 	(sql: string, params: ActualParams<P>, hints: P, ret: R) => Promise<ActualRet<R>>,
#[deno_core::op2(async)]
#[serde]
async fn op_sql_fetch_one(
	state: Rc<RefCell<OpState>>,
	#[string] sql: String,
	#[serde] raw_params: Vec<serde_json::Value>,
	#[serde] hints: Vec<PgTypeHint>,
	#[serde] ret: RetHint,
) -> Result<serde_json::Value, deno_error::JsErrorBox> {
	let state = state.as_ref();
	demand_external_allowed(state)?;
	let state = state.borrow();
	// TODO we're connecting every time here, which seems necessary for security, but terrible for performance
	let fn_config = deno_core::_ops::opstate_borrow::<PgConfig>(&state);
	let (client, connection) = fn_config.connect(postgres::NoTls).await.map_err(run_err)?;
	tokio::spawn(async move { if let Err(e) = connection.await { log::error!("DB connection error: {}", e); } });

	let params = prepare_params(raw_params, hints).map_err(run_err)?;
	let row_stream = client.query_typed_raw(&sql, params).await.map_err(run_err)?;

	use futures_util::{pin_mut, TryStreamExt};
	pin_mut!(row_stream);
	let mut first = None;
	while let Some(row) = row_stream.try_next().await.map_err(run_err)? {
		if first.is_some() {
			return Err(deno_error::JsErrorBox::generic("query returned more than 1 row".to_string()));
		}

		first = Some(convert_row(row, &ret).map_err(run_err)?);
	}

	first.ok_or_else(|| deno_error::JsErrorBox::generic("query returned no rows".to_string()))
}

// op_sql_fetch_optional: <P extends ParamHint[], R extends FullRetHint>
// 	(sql: string, params: ActualParams<P>, hints: P, ret: R) => Promise<ActualRet<R> | null>,
#[deno_core::op2(async)]
#[serde]
async fn op_sql_fetch_optional(
	state: Rc<RefCell<OpState>>,
	#[string] sql: String,
	#[serde] raw_params: Vec<serde_json::Value>,
	#[serde] hints: Vec<PgTypeHint>,
	#[serde] ret: RetHint,
) -> Result<Option<serde_json::Value>, deno_error::JsErrorBox> {
	let state = state.as_ref();
	demand_external_allowed(state)?;
	let state = state.borrow();
	// TODO we're connecting every time here, which seems necessary for security, but terrible for performance
	let fn_config = deno_core::_ops::opstate_borrow::<PgConfig>(&state);
	let (client, connection) = fn_config.connect(postgres::NoTls).await.map_err(run_err)?;
	tokio::spawn(async move { if let Err(e) = connection.await { log::error!("DB connection error: {}", e); } });

	let params = prepare_params(raw_params, hints).map_err(run_err)?;
	let row_stream = client.query_typed_raw(&sql, params).await.map_err(run_err)?;

	use futures_util::{pin_mut, TryStreamExt};
	pin_mut!(row_stream);
	let mut first = None;
	while let Some(row) = row_stream.try_next().await.map_err(run_err)? {
		if first.is_some() {
			return Err(deno_error::JsErrorBox::generic("query returned more than 1 row".to_string()));
		}

		first = Some(convert_row(row, &ret).map_err(run_err)?);
	}

	Ok(first)
}

// op_sql_execute_statement: <P extends ParamHint[]>
// 	(sql: string, params: ActualParams<P>, hints: P) => Promise<number>,
#[deno_core::op2(async)]
async fn op_sql_execute_statement(
	state: Rc<RefCell<OpState>>,
	#[string] sql: String,
	#[serde] raw_params: Vec<serde_json::Value>,
	#[serde] hints: Vec<PgTypeHint>,
) -> Result<u32, deno_error::JsErrorBox> {
	let state = state.as_ref();
	demand_external_allowed(state)?;
	let state = state.borrow();
	// TODO we're connecting every time here, which seems necessary for security, but terrible for performance
	let fn_config = deno_core::_ops::opstate_borrow::<PgConfig>(&state);
	let (client, connection) = fn_config.connect(postgres::NoTls).await.map_err(run_err)?;
	tokio::spawn(async move { if let Err(e) = connection.await { log::error!("DB connection error: {}", e); } });

	let params = prepare_params(raw_params, hints).map_err(run_err)?;
	let row_stream = client.query_typed_raw(&sql, params).await.map_err(run_err)?;
	use futures_util::{pin_mut, TryStreamExt};
	pin_mut!(row_stream);
	// TODO more elegant way to throw away all the rows?
	while let Some(_) = row_stream.try_next().await.map_err(run_err)? {}
	let rows_affected = row_stream.rows_affected().unwrap_or(0).try_into().map_err(js_err)?;
	Ok(rows_affected)
}

// op_sql_execute_statements: (sql: string) => Promise<void>,
#[deno_core::op2(async)]
async fn op_sql_execute_statements(
	state: Rc<RefCell<OpState>>,
	#[string] sql: String,
) -> Result<(), deno_error::JsErrorBox> {
	let state = state.as_ref();
	demand_external_allowed(state)?;
	let state = state.borrow();
	// TODO we're connecting every time here, which seems necessary for security, but terrible for performance
	let fn_config = deno_core::_ops::opstate_borrow::<PgConfig>(&state);
	let (mut client, connection) = fn_config.connect(postgres::NoTls).await.map_err(run_err)?;
	tokio::spawn(async move { if let Err(e) = connection.await { log::error!("DB connection error: {}", e); } });

	let txn = client.transaction().await.map_err(run_err)?;
	txn.batch_execute(&sql).await.map_err(run_err)?;
	txn.commit().await.map_err(run_err)?;

	Ok(())
}

pub type FnMap = std::collections::HashMap<String, Fn>;

#[derive(Debug)]
pub enum Fn {
	Action(v8::Global<v8::Function>),
	View(v8::Global<v8::Function>),
}

// op_register_fn: <T>(name: string, isAction: boolean, func: (arg: T, userId: string | null) => Promise<string | void>) => void,
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

// // op_create_recurring_action: (description: string, start: string, recurrenceGranularity: RecurrenceGranularity, recurrenceMultiplier: number, action_name: string, action_arg: JsonValue) => Promise<string>,
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
	let server_pg_config = deno_core::_ops::opstate_borrow::<ServerPgConfig>(&state).0.clone();
	let server_role_pool = deno_core::_ops::opstate_borrow::<PgPool>(&state);
	let scheduled_action_queue = deno_core::_ops::opstate_borrow::<ScheduledActionQueue>(&state);
	let current_full_path = deno_core::_ops::opstate_borrow::<String>(&state);

	let client = server_role_pool.get().await.map_err(run_err)?;
	let action = queries::scheduled::create_detached_recurring_action()
		.bind(&client, &current_full_path, &description, &start.naive_utc(), &recurrence_granularity, &recurrence_multiplier, &action_name, &action_arg)
		.one().await.map_err(run_err)?;

	scheduled_action_queue.queue(
		server_role_pool.clone(), server_pg_config,
		ScheduledActionKind::DetachedRecurring, action.id, action.next_scheduled_time.and_utc(),
	);

	Ok(action.id.to_string())
}

// op_remove_recurring_action: (uuid: string) => Promise<void>,
#[deno_core::op2(async)]
#[string]
async fn op_remove_recurring_action(
	state: Rc<RefCell<OpState>>,
	#[serde] scheduled_action_uuid: Uuid,
) -> Result<(), deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;
	let state = state.as_ref().borrow();
	let server_role_pool = deno_core::_ops::opstate_borrow::<PgPool>(&state);

	let client = server_role_pool.get().await.map_err(run_err)?;
	queries::scheduled::remove_detached_recurring_action()
		.bind(&client, &scheduled_action_uuid).await.map_err(run_err)?;

	Ok(())
}


// op_schedule_action: (description: string, scheduled_time: string, action_name: string, action_arg: JsonValue) => Promise<string>,
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
	let server_role_pool = deno_core::_ops::opstate_borrow::<PgPool>(&state);
	let server_pg_config = deno_core::_ops::opstate_borrow::<ServerPgConfig>(&state).0.clone();
	let scheduled_action_queue = deno_core::_ops::opstate_borrow::<ScheduledActionQueue>(&state);
	let current_full_path = deno_core::_ops::opstate_borrow::<String>(&state);

	let client = server_role_pool.get().await.map_err(run_err)?;
	let id = queries::scheduled::create_detached_scheduled_action()
		.bind(&client, &current_full_path, &description, &scheduled_time.fixed_offset(), &action_name, &action_arg)
		.one().await.map_err(run_err)?;

	scheduled_action_queue.queue(
		server_role_pool.clone(), server_pg_config,
		ScheduledActionKind::DetachedScheduled, id, scheduled_time,
	);

	Ok(id.to_string())
}

// op_unschedule_action: (uuid: string) => Promise<void>,
#[deno_core::op2(async)]
#[string]
async fn op_unschedule_action(
	state: Rc<RefCell<OpState>>,
	#[serde] scheduled_action_uuid: Uuid,
) -> Result<(), deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;
	let state = state.as_ref().borrow();
	let server_role_pool = deno_core::_ops::opstate_borrow::<PgPool>(&state);

	let client = server_role_pool.get().await.map_err(run_err)?;
	queries::scheduled::remove_detached_scheduled_action()
		.bind(&client, &scheduled_action_uuid).await.map_err(run_err)?;

	Ok(())
}

// op_enroll_member: (email: string) => Promise<string>,
#[deno_core::op2(async)]
#[string]
async fn op_enroll_member(
	state: Rc<RefCell<OpState>>,
	#[string] email: String,
) -> Result<String, deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;
	let state = state.as_ref().borrow();
	let server_role_pool = deno_core::_ops::opstate_borrow::<PgPool>(&state);

	let client = server_role_pool.get().await.map_err(run_err)?;
	let id = queries::members::enroll_member()
		.bind(&client, &email)
		.one().await.map_err(run_err)?;

	// TODO perhaps at some point there's a notification email sent to this person here or something

	Ok(id.to_string())
}

// op_remove_member_by_email: (email: string) => Promise<void>,
#[deno_core::op2(async)]
async fn op_remove_member_by_email(
	state: Rc<RefCell<OpState>>,
	#[string] email: String,
) -> Result<(), deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;
	let state = state.as_ref().borrow();
	let server_role_pool = deno_core::_ops::opstate_borrow::<PgPool>(&state);

	let client = server_role_pool.get().await.map_err(run_err)?;
	queries::members::remove_member_by_email()
		.bind(&client, &email).await.map_err(run_err)?;

	// TODO perhaps at some point there's a notification email sent to this person here or something

	Ok(())
}
// op_remove_member_by_uuid: (uuid: string) => Promise<void>,
#[deno_core::op2(async)]
async fn op_remove_member_by_uuid(
	state: Rc<RefCell<OpState>>,
	#[serde] member_uuid: Uuid,
) -> Result<(), deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;
	let state = state.as_ref().borrow();

	let server_role_pool = deno_core::_ops::opstate_borrow::<PgPool>(&state);
	let client = server_role_pool.get().await.map_err(run_err)?;
	queries::members::remove_member_by_uuid()
		.bind(&client, &member_uuid).await.map_err(run_err)?;

	// TODO perhaps at some point there's a notification email sent to this person here or something

	Ok(())
}

// TODO op_add_members_to_ruleset: (full_path: string, uuids: string[]) => Promise<void>,
// TODO op_add_members_to_ruleset_by_condition: (full_path: string, condition: string) => Promise<void>,
// TODO op_remove_members_from_ruleset: (full_path: string, uuids: string[]) => Promise<void>,

// op_propose_self_replacement: (candidate: CandidateRuleset) => Promise<string>,
#[deno_core::op2(async, reentrant)]
#[string]
async fn op_propose_self_replacement(
	state: Rc<RefCell<OpState>>,
	#[serde] candidate: rulesets::CandidateRuleset,
) -> Result<String, deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;
	let state = state.as_ref().borrow();
	let current_full_path = deno_core::_ops::opstate_borrow::<String>(&state);
	let server_pg_config = deno_core::_ops::opstate_borrow::<ServerPgConfig>(&state);
	let server_role_pool = deno_core::_ops::opstate_borrow::<PgPool>(&state);
	let server_pg_client = server_role_pool.get().await.map_err(run_err)?;

	let candidate_uuid = rulesets::propose_candidate_ruleset().await.map_err(run_err)?;

	Ok(candidate_uuid.to_string())
}

// op_create_child_ruleset: (
// 	name: string, initial: ConcreteRuleset,
// 	// tables in the parent ruleset that the view role of the child ruleset are granted select and the migrator role is granted references to
// 	allowed_view_tables: string[],
// 	// functions in the parent ruleset that the action role of the child ruleset are granted execute
// 	allowed_action_functions: string[],
// ) => Promise<string>,
#[deno_core::op2(async, reentrant)]
#[string]
async fn op_create_child_ruleset(
	state: Rc<RefCell<OpState>>,
	#[string] name: String,
	#[serde] initial: rulesets::ConcreteRuleset,
	#[serde] allowed_view_tables: Vec<String>,
	#[serde] allowed_action_functions: Vec<String>,
) -> String {
	unimplemented!()
}

// op_delete_child_ruleset: (name: string) => Promise<void>,
#[deno_core::op2(async, reentrant)]
#[string]
async fn op_delete_child_ruleset(
	state: Rc<RefCell<OpState>>,
	#[string] name: String,
) -> String {
	unimplemented!()
}

// // this is just the child version of op_propose_self_replacement
// op_propose_child_replacement: (name: string, candidate: CandidateRuleset) => Promise<string>,
// // the table with candidate_id already has the name and full_path etc to know where it's headed
// op_replace_child: (candidate_id: string) => Promise<void>,

#[derive(Clone)]
pub struct ScheduledActionQueue {
	send: tokio::sync::mpsc::UnboundedSender<ScheduledAction>,
}

type ScheduledAction = (PgPool, PgConfig, ScheduledActionKind, Uuid, tokio::time::Instant);

impl ScheduledActionQueue {
	pub fn new() -> Self {
		let (send, mut recv) = tokio::sync::mpsc::unbounded_channel();
		let spawner = Self { send };

		let rt = tokio::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.unwrap();

		let thread_spawner = spawner.clone();
		std::thread::spawn(move || {
			let local = tokio::task::LocalSet::new();

			local.spawn_local(async move {
				while let Some(scheduled_action) = recv.recv().await {
					tokio::task::spawn_local(do_scheduled_action(scheduled_action, thread_spawner.clone()));
				}
			});

			rt.block_on(local);
		});

		spawner
	}

	pub fn queue(
		&self,
		server_role_pool: PgPool,
		server_pg_config: PgConfig,
		scheduled_action_kind: ScheduledActionKind,
		scheduled_action_uuid: Uuid,
		scheduled_time: chrono::DateTime<chrono::Utc>,
	// ) -> Result<(), tokio::sync::mpsc::error::SendError<ScheduledAction>> {
	) {
		let scheduled_time = compute_time_until(scheduled_time);
		let scheduled_action = (
			server_role_pool, server_pg_config,
			scheduled_action_kind, scheduled_action_uuid, scheduled_time,
		);
		if let Err(e) = self.send.send(scheduled_action) {
			log::error!("couldn't send scheduled action? {}", e);
		};
	}
}

fn compute_time_until(scheduled_time: chrono::DateTime<chrono::Utc>) -> tokio::time::Instant {
	let duration_until = scheduled_time.signed_duration_since(chrono::Utc::now()).to_std().unwrap();
	tokio::time::Instant::now() + duration_until
}

async fn do_scheduled_action(scheduled_action: ScheduledAction, spawner: ScheduledActionQueue) {
	let (
		server_role_pool, server_pg_config,
		scheduled_action_kind, scheduled_action_uuid, scheduled_time,
	) = scheduled_action;

	tokio::time::sleep_until(scheduled_time).await;

	log::info!("attempting action {} of type {}", scheduled_action_uuid, &scheduled_action_kind);
	let result = match scheduled_action_kind {
		// ScheduledActionKind::Recurring => {
		// 	execute_recurring_action(server_role_pool, server_pg_config, false, scheduled_action_uuid).await
		// },
		ScheduledActionKind::DetachedRecurring => {
			execute_recurring_action(&spawner, server_role_pool, server_pg_config, scheduled_action_uuid).await
		},
		ScheduledActionKind::DetachedScheduled => {
			execute_scheduled_action(spawner.clone(), server_role_pool, server_pg_config, scheduled_action_uuid).await
			// if let Err(e) = result {
			// 	error!("failed to execute scheduled action {}; {}", scheduled_action_uuid, e);
			// }
		},
	};

	if let Err(e) = result {
		log::error!("error when queueing scheduled action {}: {}", scheduled_action_uuid, e);
	}
}


pub async fn execute_recurring_action(
	scheduled_action_queue: &ScheduledActionQueue,
	server_role_pool: PgPool,
	server_pg_config: PgConfig,
	// is_detached: bool,
	scheduled_action_uuid: uuid::Uuid,
) -> Result<(), RuntimeError> {
	// let scheduled_action_kind = if is_detached { ScheduledActionKind::DetachedRecurring } else { ScheduledActionKind::Recurring };
	let scheduled_action_kind = ScheduledActionKind::DetachedRecurring;

	let client = server_role_pool.get().await?;
	let action = queries::scheduled::acquire_detached_recurring_action()
		.bind(&client, &scheduled_action_uuid).opt().await?;
	// if is_detached {
	// } else {
	// 	unimplemented!()
	// };

	match action {
		None => { info!("wasn't able to acquire scheduled action {}", scheduled_action_uuid); Ok(()) },
		Some(action) => {
			let scheduled = action.next_scheduled_time.and_utc();
			let now = chrono::Utc::now();
			log::info!("{} scheduled: {}; actual: {}; {}", scheduled_action_uuid, scheduled, now, action.description);
			if scheduled > now {
				log::info!("{} not doing it yet", scheduled_action_uuid);
				scheduled_action_queue.queue(server_role_pool, server_pg_config, scheduled_action_kind, scheduled_action_uuid, scheduled);
				return Ok(())
			}

			run_action(
				action.full_path, action.code, &action.action_name, &action.action_pass, &action.migrator_pass, action.arg,
				server_pg_config.clone(), &server_role_pool, scheduled_action_queue.clone(),
			).await?;

			let next_scheduled_time = queries::scheduled::release_detached_recurring_action()
				.bind(&client, &scheduled_action_uuid).one().await?.and_utc();
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

			scheduled_action_queue.queue(server_role_pool, server_pg_config, scheduled_action_kind, scheduled_action_uuid, next_scheduled_time);

			Ok(())
		},
	}
}

pub async fn execute_scheduled_action(
	scheduled_action_queue: ScheduledActionQueue,
	server_role_pool: PgPool,
	server_pg_config: PgConfig,
	scheduled_action_uuid: uuid::Uuid,
) -> Result<(), RuntimeError> {
	let client = server_role_pool.get().await?;
	let action = queries::scheduled::acquire_detached_scheduled_action()
		.bind(&client, &scheduled_action_uuid)
		.opt().await?;

	match action {
		None => { info!("wasn't able to acquire scheduled action {}", scheduled_action_uuid); Ok(()) },
		Some(action) => {
			let scheduled = action.scheduled_time.to_utc();
			let now = chrono::Utc::now();
			let diff = scheduled - now;
			log::info!("{} ({}): scheduled: {}; actual: {}; difference: {}", scheduled_action_uuid, action.description, scheduled, now, diff);

			run_action(
				action.full_path, action.code, &action.action_name, &action.action_pass, &action.migrator_pass, action.arg,
				server_pg_config.clone(), &server_role_pool, scheduled_action_queue,
			).await?;

			queries::scheduled::remove_detached_scheduled_action()
				.bind(&client, &scheduled_action_uuid).await?;

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
	server_pg_config: PgConfig,
	server_role_pool: &PgPool,
	scheduled_action_queue: ScheduledActionQueue,
) -> Result<(), RuntimeError> {
	let mut migrator_role_config = server_pg_config.clone();
	let migrator_role = format_ruleset_role(&current_full_path, RoleType::Migrator);
	migrator_role_config.user(&migrator_role).password(migrator_pass);
	let mut action_role_config = server_pg_config.clone();
	let action_role = format_ruleset_role(&current_full_path, RoleType::Action);
	action_role_config.user(&action_role).password(action_pass);

	let new_ruleset_id = run_function::<Option<String>>(
		current_full_path, ruleset_code, action_name, arg, FnType::Action,
		action_role_config, server_pg_config, server_role_pool.clone(), scheduled_action_queue,
	).await?;

	if let Some(new_ruleset_id) = new_ruleset_id {
		let new_ruleset_id = new_ruleset_id.parse::<uuid::Uuid>()?;
		info!("apply_candidate {new_ruleset_id}");
		let client = server_role_pool.get().await?;
		rulesets::replace_ruleset(&client, migrator_role_config, new_ruleset_id).await?;
	}

	Ok(())
}

pub async fn run_view(
	current_full_path: String,
	ruleset_code: String,
	view_name: &str,
	view_pass: &str,
	query: serde_json::Value,
	server_pg_config: PgConfig,
	server_role_pool: &PgPool,
	scheduled_action_queue: ScheduledActionQueue,
) -> Result<String, RuntimeError> {
	let view_role = format_ruleset_role(&current_full_path, RoleType::View);
	let mut view_role_config = server_pg_config.clone();
	view_role_config.user(view_role).password(view_pass);

	run_function(
		current_full_path, ruleset_code, view_name, query, FnType::View,
		view_role_config, server_pg_config, server_role_pool.clone(), scheduled_action_queue,
	).await
}

async fn run_function<'r, V: deno_core::serde::Deserialize<'r>>(
	current_full_path: String,
	ruleset_code: String,
	function_name: &str,
	function_arg: serde_json::Value,
	function_type: FnType,
	fn_role_config: PgConfig,
	server_pg_config: PgConfig,
	server_role_pool: PgPool,
	scheduled_action_queue: ScheduledActionQueue,
) -> Result<V, RuntimeError> {
	let mut runtime = Runtime::new(&ruleset_code).await.map_err(|e| RuntimeError::OtherError(e.to_string()))?;

	// fn_role_config encodes the user, and therefore the role and powers of the connection
	let fn_map = runtime.take_fn_map();
	let function = fn_map.get(function_name)
		.ok_or_else(|| RuntimeError::OtherError(format!("{} '{}' not found", function_type, function_name)))?;

	let function = match (function_type, function) {
		(FnType::Action, Fn::Action(function)) => function,
		(FnType::View, Fn::View(function)) => function,
		_ => { return Err(RuntimeError::OtherError(format!("'{}' isn't of type {}", function_name, function_type))) },
	};

	let function_arg = {
		let mut scope = runtime.js_runtime.handle_scope();
		let function_arg = deno_core::serde_v8::to_v8(&mut scope, function_arg)?;
		v8::Global::new(&mut scope, function_arg)
	};

	runtime.set_external_allowed(true);
	runtime.set_action_queue(scheduled_action_queue);
	runtime.set_server_pool(server_role_pool);
	runtime.set_server_config(server_pg_config);
	runtime.set_fn_config(fn_role_config);
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
