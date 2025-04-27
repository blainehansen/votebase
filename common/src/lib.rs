#[macro_use] extern crate log;

pub mod runtime;

pub type PgPool = sqlx::Pool<sqlx::Postgres>;
pub type PgOpt = sqlx::postgres::PgConnectOptions;

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
// use tokio_postgres::Client;
use serde_json::Value as Val;

pub fn convert_pg_row(row: tokio_postgres::Row, want_scalar: bool) -> Result<Val, deno_error::JsErrorBox> {
	let columns = row.columns();
	if want_scalar {
		if columns.len() != 1 {
			return Err(deno_error::JsErrorBox::type_error("query doesn't return single scalar value"))
		}

		let row = columns[0];
	}

	Ok(())
}

pub fn convert_pg_column(
	row: tokio_postgres::Row,
	column: tokio_postgres::Column,
	index: usize,
) -> Result<Val, deno_error::JsErrorBox> {
	use tokio_postgres::types::{Type, Kind};

	let type_ = column.type_();
	match type_.kind() {
		Kind::Simple => {
			if type_ == &Type::BOOL {
				Ok(Val::Bool(row.try_get(index).map_err(js_err)?))
			}
			else { Err(deno_error::JsErrorBox::generic("don't know how to convert")) }

		},
		Kind::Enum(_) => { Ok(Val::String(row.try_get(index).map_err(js_err)?)) },
		Kind::Array(inner) => {},
		Kind::Range(inner) => {},
		Kind::Composite(fields) => {},
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

// 	let (client, connection) = tokio_postgres::connect(url, tokio_postgres::NoTls).await.unwrap();

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
