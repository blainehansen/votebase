#[macro_use] extern crate log;

pub mod runtime;

pub use votebase_queries::{queries, deadpool_postgres as deadpool, tokio_postgres as postgres};

pub type PgPool = deadpool::Pool;
pub type PgClient = deadpool::Client;
pub type PgConfig = postgres::Config;

#[derive(Copy, Clone, Debug)]
pub enum RoleType { Migrator, Action, View }
impl std::fmt::Display for RoleType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			RoleType::Migrator => write!(f, "migrator"),
			RoleType::Action => write!(f, "action"),
			RoleType::View => write!(f, "view"),
		}
	}
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

pub fn format_ruleset_schema(full_path: &str) -> String {
	format!("ruleset:{full_path}")
}

pub fn format_ruleset_role(full_path: &str, role_type: RoleType) -> String {
	format!("role:{full_path}|{role_type}")
}


#[derive(Copy, Clone, Debug)]
pub enum ScheduledActionKind { /*Recurring,*/ DetachedRecurring, DetachedScheduled }
impl std::fmt::Display for ScheduledActionKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			// ScheduledActionKind::Recurring => write!(f, "Recurring"),
			ScheduledActionKind::DetachedRecurring => write!(f, "DetachedRecurring"),
			ScheduledActionKind::DetachedScheduled => write!(f, "DetachedScheduled"),
		}
	}
}

use runtime::js_err;
use serde_json::Value as Val;

pub fn convert_pg_row(row: postgres::Row, want_scalar: bool) -> Result<Val, deno_error::JsErrorBox> {
	let columns = row.columns();
	if want_scalar {
		if columns.len() != 1 {
			return Err(deno_error::JsErrorBox::type_error("query doesn't return single scalar value"))
		}

		let col = &columns[0];
		return convert_pg_column(&row, col, 0);
	}

	Ok(Val::Object(
		columns.into_iter().enumerate()
			.map(|(index, col)| Ok::<_, deno_error::JsErrorBox>((col.name().to_string(), convert_pg_column(&row, col, index)?)))
			.collect::<Result<_, _>>()?
	))
}

const DATETIME_FORMAT: chrono::format::strftime::StrftimeItems = chrono::format::strftime::StrftimeItems::new("%+");
const DATE_FORMAT: chrono::format::strftime::StrftimeItems = chrono::format::strftime::StrftimeItems::new("%F");

pub fn convert_pg_column(
	row: &postgres::Row,
	column: &postgres::Column,
	index: usize,
) -> Result<Val, deno_error::JsErrorBox> {
	use postgres::types::{Kind, ToSql};

	let type_ = column.type_();
	// https://docs.rs/postgres-types/0.2.9/src/postgres_types/lib.rs.html#456
	match type_.kind() {
		Kind::Simple => {
			// /// | `bool`                            | BOOL                                          |
			if <bool as ToSql>::accepts(type_) {
				let value: Option<bool> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, Val::Bool))
			}
			// /// | `i8`                              | "char"                                        |
			else if <i8 as ToSql>::accepts(type_) {
				let value: Option<i8> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| v.into()))
			}
			// /// | `i16`                             | SMALLINT, SMALLSERIAL                         |
			else if <i16 as ToSql>::accepts(type_) {
				let value: Option<i16> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| v.into()))
			}
			// /// | `i32`                             | INT, SERIAL                                   |
			else if <i32 as ToSql>::accepts(type_) {
				let value: Option<i32> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| v.into()))
			}
			// /// | `i64`                             | BIGINT, BIGSERIAL                             |
			else if <i64 as ToSql>::accepts(type_) {
				let value: Option<i64> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| v.into()))
			}
			// /// | `u32`                             | OID                                           |
			else if <u32 as ToSql>::accepts(type_) {
				let value: Option<u32> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| v.into()))
			}
			// /// | `f32`                             | REAL                                          |
			else if <f32 as ToSql>::accepts(type_) {
				let value: Option<f32> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| v.into()))
			}
			// /// | `f64`                             | DOUBLE PRECISION                              |
			else if <f64 as ToSql>::accepts(type_) {
				let value: Option<f64> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| v.into()))
			}
			// /// | `&str`/`String`                   | VARCHAR, CHAR(n), TEXT, CITEXT, NAME, UNKNOWN |
			// /// |                                   | LTREE, LQUERY, LTXTQUERY                      |
			else if <String as ToSql>::accepts(type_) {
				let value: Option<String> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, Val::String))
			}
			// /// | `&[u8]`/`Vec<u8>`                 | BYTEA                                         |
			else if <Vec<u8> as ToSql>::accepts(type_) {
				let value: Option<Vec<u8>> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| Val::Array(
					v.into_iter().map(|n| Val::Number(serde_json::Number::from(n as u16))).collect()
				)))
			}
			// /// | `HashMap<String, Option<String>>` | HSTORE                                        |
			else if <std::collections::HashMap<String, Option<String>> as ToSql>::accepts(type_) {
				let value: Option<std::collections::HashMap<String, Option<String>>> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| Val::Object(
					v.into_iter().map(|(k, v)| (k, v.map_or(Val::Null, Val::String))).collect()
				)))
			}
			// /// | `IpAddr`                          | INET
			else if <std::net::IpAddr as ToSql>::accepts(type_) {
				let value: Option<std::net::IpAddr> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| Val::String(v.to_string())))
			}
			// /// | `chrono::NaiveDateTime`         | TIMESTAMP                           |
			else if <chrono::NaiveDateTime as ToSql>::accepts(type_) {
				let value: Option<chrono::NaiveDateTime> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| Val::String(v.format_with_items(DATETIME_FORMAT).to_string())))
			}
			// /// | `chrono::NaiveDate`             | DATE                                |
			else if <chrono::NaiveDate as ToSql>::accepts(type_) {
				let value: Option<chrono::NaiveDate> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| Val::String(v.format_with_items(DATE_FORMAT).to_string())))
			}
			// https://docs.rs/chrono/latest/chrono/format/strftime/index.html#fn7
			// /// | `chrono::NaiveTime`             | TIME                                |
			// /// | `chrono::DateTime<Utc>`         | TIMESTAMP WITH TIME ZONE            |
			// /// | `chrono::DateTime<Local>`       | TIMESTAMP WITH TIME ZONE            |
			// /// | `chrono::DateTime<FixedOffset>` | TIMESTAMP WITH TIME ZONE            |
			else if <chrono::DateTime<chrono::Utc> as ToSql>::accepts(type_) {
				let value: Option<chrono::DateTime<chrono::Utc>> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| Val::String(v.to_rfc3339())))
			}
			// /// | `cidr::IpCidr`                  | CIDR                                |
			// /// | `cidr::IpInet`                  | INET                                |
			// /// | `eui48::MacAddress`             | MACADDR                             |
			// /// | `geo_types::Point<f64>`         | POINT                               |
			// /// | `geo_types::Rect<f64>`          | BOX                                 |
			// /// | `geo_types::LineString<f64>`    | PATH                                |
			// /// | `bit_vec::BitVec`               | BIT, VARBIT                         |
			// /// | `eui48::MacAddress`             | MACADDR                             |
			// /// | `cidr::InetCidr`                | CIDR                                |
			// /// | `cidr::InetAddr`                | INET                                |

			// /// | `uuid::Uuid`                    | UUID                                |
			else if <uuid::Uuid as ToSql>::accepts(type_) {
				let value: Option<uuid::Uuid> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| Val::String(v.to_string())))
			}
			// /// | `serde_json::Value`             | JSON, JSONB                         |
			else if <serde_json::Value as ToSql>::accepts(type_) {
				let value: Option<serde_json::Value> = row.try_get(index).map_err(js_err)?;
				Ok(value.unwrap_or(Val::Null))
			}
			else { Err(deno_error::JsErrorBox::generic(format!("don't know how to convert {}", type_.name()))) }
		},
		Kind::Enum(_) => {
			let value: Option<String> = row.try_get(index).map_err(js_err)?;
			Ok(value.map_or(Val::Null, Val::String))
		},
		// Kind::Array(inner) => {},
		// Kind::Range(inner) => {},
		// Kind::Composite(fields) => {},
		// TODO add oid?
		// Kind::Multirange(inner) => {},
		// Kind::Domain(inner) => {},
		// Kind::Pseudo => {},
		_ => { Err(deno_error::JsErrorBox::generic(format!("don't know how to convert {}", type_.name()))) },
	}
}

fn make_params(params: Option<Vec<serde_json::Value>>) -> Vec<Box<(dyn postgres::types::ToSql + Sync)>> {
	match params {
		None => vec![],
		Some(params) => {
			let mut final_params: Vec<Box<(dyn postgres::types::ToSql + Sync)>> = vec![];
			for param in params {
				match param {
					serde_json::Value::Null => { final_params.push(Box::new(None::<bool>)); },
					serde_json::Value::Bool(param) => { final_params.push(Box::new(param)); },
					serde_json::Value::Number(param) => {
						if param.is_f64() {
							final_params.push(Box::new(param.as_f64()));
						} else {
							final_params.push(Box::new(param.as_i64()));
						}
					},
					serde_json::Value::String(param) => { final_params.push(Box::new(param)); },
					serde_json::Value::Array(param) => { final_params.push(Box::new(param)); },
					param => { final_params.push(Box::new(param)); },
				}
			}

			final_params
		},
	}
}
