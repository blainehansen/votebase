use std::{cell::RefCell, collections::HashMap, rc::Rc};
use deno_core::OpState;

use super::{RuntimeError, demand_external_allowed, run_err, js_err};
use crate::{postgres, FnRolePg};

#[derive(serde::Deserialize, Debug)]
#[serde(variant_identifier)]
pub enum PgTypeHint {
	Json, Bool, Text, Bytea, Hstore,
	I16, I32, I64, F32, F64,
	// | `u32`: OID
}

pub fn prepare_param(
	raw_param: serde_json::Value,
	hint: PgTypeHint,
) -> Result<(Box<dyn postgres::types::ToSql + Sync>, postgres::types::Type), RuntimeError> {
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
) -> Result<Vec<(Box<dyn postgres::types::ToSql + Sync>, postgres::types::Type)>, RuntimeError> {
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
pub async fn op_sql_fetch_all(
	state: Rc<RefCell<OpState>>,
	#[string] sql: String,
	// TODO the ambition here is to no longer have this no longer be a vec of serde, but one of v8::Object or Value or whatever
	// and then directly decoding those values in the functions
	// similar with the return value, going directly from the rust values given by tokio postgres to v8 ones
	#[serde] raw_params: Vec<serde_json::Value>,
	#[serde] hints: Vec<PgTypeHint>,
	#[serde] ret: RetHint,
) -> Result<Vec<serde_json::Value>, deno_error::JsErrorBox> {
	let state = state.as_ref().borrow();
	demand_external_allowed(&state)?;
	let fn_role_pg = state.borrow::<FnRolePg>();
	let client = fn_role_pg.get_client().await.map_err(run_err)?;

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
pub async fn op_sql_fetch_one(
	state: Rc<RefCell<OpState>>,
	#[string] sql: String,
	#[serde] raw_params: Vec<serde_json::Value>,
	#[serde] hints: Vec<PgTypeHint>,
	#[serde] ret: RetHint,
) -> Result<serde_json::Value, deno_error::JsErrorBox> {
	let state = state.as_ref().borrow();
	demand_external_allowed(&state)?;
	let fn_role_pg = state.borrow::<FnRolePg>();
	let client = fn_role_pg.get_client().await.map_err(run_err)?;

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
pub async fn op_sql_fetch_optional(
	state: Rc<RefCell<OpState>>,
	#[string] sql: String,
	#[serde] raw_params: Vec<serde_json::Value>,
	#[serde] hints: Vec<PgTypeHint>,
	#[serde] ret: RetHint,
) -> Result<Option<serde_json::Value>, deno_error::JsErrorBox> {
	let state = state.as_ref().borrow();
	demand_external_allowed(&state)?;
	let fn_role_pg = state.borrow::<FnRolePg>();
	let client = fn_role_pg.get_client().await.map_err(run_err)?;

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
pub async fn op_sql_execute_statement(
	state: Rc<RefCell<OpState>>,
	#[string] sql: String,
	#[serde] raw_params: Vec<serde_json::Value>,
	#[serde] hints: Vec<PgTypeHint>,
) -> Result<u32, deno_error::JsErrorBox> {
	let state = state.as_ref().borrow();
	demand_external_allowed(&state)?;
	let fn_role_pg = state.borrow::<FnRolePg>();
	let client = fn_role_pg.get_client().await.map_err(run_err)?;

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
pub async fn op_sql_execute_statements(
	state: Rc<RefCell<OpState>>,
	#[string] sql: String,
) -> Result<(), deno_error::JsErrorBox> {
	let state = state.as_ref().borrow();
	demand_external_allowed(&state)?;
	let fn_role_pg = state.borrow::<FnRolePg>();
	let mut client = fn_role_pg.get_client().await.map_err(run_err)?;

	let txn = client.transaction().await.map_err(run_err)?;
	txn.batch_execute(&sql).await.map_err(run_err)?;
	txn.commit().await.map_err(run_err)?;

	Ok(())
}
