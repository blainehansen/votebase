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

	unimplemented!()
}


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
			if <bool as ToSql>::accepts(type_) {
				let value: Option<bool> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, Val::Bool))
			}
			else if <i8 as ToSql>::accepts(type_) {
				let value: Option<i8> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| v.into()))
			}
			else if <i16 as ToSql>::accepts(type_) {
				let value: Option<i16> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| v.into()))
			}
			else if <i32 as ToSql>::accepts(type_) {
				let value: Option<i32> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| v.into()))
			}
			else if <i64 as ToSql>::accepts(type_) {
				let value: Option<i64> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| v.into()))
			}
			else if <u32 as ToSql>::accepts(type_) {
				let value: Option<u32> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| v.into()))
			}
			else if <f32 as ToSql>::accepts(type_) {
				let value: Option<f32> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| v.into()))
			}
			else if <f64 as ToSql>::accepts(type_) {
				let value: Option<f64> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, |v| v.into()))
			}
			else if <String as ToSql>::accepts(type_) {
				let value: Option<String> = row.try_get(index).map_err(js_err)?;
				Ok(value.map_or(Val::Null, Val::String))
			}
			// /// | `&[u8]`/`Vec<u8>`                 | BYTEA                                         |
			// /// | `HashMap<String, Option<String>>` | HSTORE                                        |
			// /// | `SystemTime`                      | TIMESTAMP, TIMESTAMP WITH TIME ZONE           |
			// /// | `IpAddr`                          | INET
			// /// | `chrono::NaiveDateTime`         | TIMESTAMP                           |
			// /// | `chrono::DateTime<Utc>`         | TIMESTAMP WITH TIME ZONE            |
			// /// | `chrono::DateTime<Local>`       | TIMESTAMP WITH TIME ZONE            |
			// /// | `chrono::DateTime<FixedOffset>` | TIMESTAMP WITH TIME ZONE            |
			// /// | `chrono::NaiveDate`             | DATE                                |
			// /// | `chrono::NaiveTime`             | TIME                                |
			// /// | `cidr::IpCidr`                  | CIDR                                |
			// /// | `cidr::IpInet`                  | INET                                |
			// /// | `time::PrimitiveDateTime`       | TIMESTAMP                           |
			// /// | `time::OffsetDateTime`          | TIMESTAMP WITH TIME ZONE            |
			// /// | `time::Date`                    | DATE                                |
			// /// | `time::Time`                    | TIME                                |
			// /// | `jiff::civil::Date`             | DATE                                |
			// /// | `jiff::civil::DateTime`         | TIMESTAMP                           |
			// /// | `jiff::civil::Time`             | TIME                                |
			// /// | `jiff::Timestamp`               | TIMESTAMP WITH TIME ZONE            |
			// /// | `eui48::MacAddress`             | MACADDR                             |
			// /// | `geo_types::Point<f64>`         | POINT                               |
			// /// | `geo_types::Rect<f64>`          | BOX                                 |
			// /// | `geo_types::LineString<f64>`    | PATH                                |
			// /// | `serde_json::Value`             | JSON, JSONB                         |
			// /// | `uuid::Uuid`                    | UUID                                |
			// /// | `bit_vec::BitVec`               | BIT, VARBIT                         |
			// /// | `eui48::MacAddress`             | MACADDR                             |
			// /// | `cidr::InetCidr`                | CIDR                                |
			// /// | `cidr::InetAddr`                | INET                                |
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

// // https://github.com/postgres/postgres/blob/master/src/include/catalog/pg_type.dat
// fn convert_unknown_pg_value(
// 	row: &sqlx::postgres::PgRow,
// 	column: &sqlx::postgres::PgColumn,
// ) -> Result<(String, serde_json::Value), deno_error::JsErrorBox> {
// 	use sqlx::{Row, Column};
// 	let type_info = column.type_info();
// 	// type_info.kind()
// 	// TODO need to map all oids (or pr against sqlx to make existing constants public), and handle edge cases like enums etc
// 	let value = if type_info.type_eq(&sqlx::postgres::PgTypeInfo::with_oid(sqlx::postgres::types::Oid(23))) {
// 		row.get::<i32, usize>(column.ordinal()).into()
// 	} else {
// 		row.get(column.ordinal())
// 	};

// 	Ok((column.name().to_owned(), value))
// }



// fn make_args(params: Option<Vec<serde_json::Value>>) -> Result<sqlx::postgres::PgArguments, sqlx::error::BoxDynError> {
// 	let mut args = sqlx::postgres::PgArguments::default();
// 	use sqlx::Arguments;
// 	match params {
// 		None => Ok(args),
// 		Some(params) => {
// 			for param in params {
// 				match param {
// 					serde_json::Value::Null => { args.add(None::<bool>)?; },
// 					serde_json::Value::Bool(param) => { args.add(param)?; },
// 					serde_json::Value::Number(param) => {
// 						if param.is_f64() {
// 							args.add(param.as_f64())?;
// 						} else {
// 							args.add(param.as_i64())?;
// 						}
// 					},
// 					serde_json::Value::String(param) => { args.add(param)?; },
// 					serde_json::Value::Array(param) => { args.add(param)?; },
// 					param => { args.add(param)?; },
// 				}
// 			}

// 			Ok(args)
// 		},
// 	}
// }



// let url = "postgresql://dev_admin_user:dev%5Fadmin%5Fpassword@localhost:5432/dev_db";

// 	let (client, connection) = postgres::connect(url, postgres::NoTls).await.unwrap();

// 	tokio::spawn(async move {
// 		if let Err(e) = connection.await {
// 			eprintln!("connection error: {}", e);
// 		}
// 	});

// 	let row = client
// 		.query_one("select $1::text", &[&"hello world"])
// 		.await.unwrap();

// 	for column in row.columns() {
// 		let name = column.name();
// 		let t = column.type_();
// 		let k = t.kind();
// 		println!("{name}: {k:?} {t:?}");
// 	}

// 	Ok(())
