use votebase_queries::{tokio_postgres as postgres, deadpool_postgres as deadpool};

#[derive(Debug)]
struct FullColumn {
	pg_type: postgres::types::Type,
	column_info: Option<ColumnInfo>,
}

#[derive(Debug)]
struct FullParam {
	name: String,
	pg_type: postgres::types::Type,
}

#[derive(Debug)]
struct FullStatement {
	name: String,
	query: String,
	params: Vec<FullParam>,
	columns: Vec<FullColumn>,
}

#[derive(Clone, Debug, postgres_from_row::FromRow)]
struct ColumnInfo {
	table_oid: u32,
	column_id: i16,
	not_null: bool,
	// has_default: bool,
}

#[derive(Debug, serde::Deserialize)]
struct RawQuery {
	name: String,
	query: String,
}

fn pg_to_ts_info(typ: &postgres::types::Type, is_not_null: bool) -> (String, String) {
	let (ts_type, ts_hint) = match typ.kind() {
		postgres::types::Kind::Simple | postgres::types::Kind::Pseudo => {
			let (ts_type, ts_hint) = base_pg_type_to_ts_info(typ);
			(ts_type.to_string(), ts_hint.to_string())
		},
		postgres::types::Kind::Enum(variants) => {
			(variants.join("|").into(), "'string'".into())
		},
		postgres::types::Kind::Array(inner) => {
			let (ts_type, ts_hint) = base_pg_type_to_ts_info(inner);
			(format!("({ts_type})[]"), ts_hint.to_string())
		},
		postgres::types::Kind::Domain(inner) => {
			let (ts_type, ts_hint) = base_pg_type_to_ts_info(inner);
			(ts_type.to_string(), ts_hint.to_string())
		},
		postgres::types::Kind::Composite(fields) => {
			let mut type_fields = vec![];
			let mut hint_fields = vec![];
			for field in fields {
				let field_name = field.name();
				let (ts_type, ts_hint) = pg_to_ts_info(field.type_(), false);
				type_fields.push(format!("{field_name}: {ts_type}"));
				hint_fields.push(format!("{field_name}: {ts_hint}"));
			}
			(
				format!("{{ {} }}", type_fields.join(", ")),
				format!("{{ {} }}", hint_fields.join(", ")),
			)
		},
		_ => panic!("don't know what to do with pg type: {}", typ),
		// postgres::types::Kind::Range(inner) => {},
		// postgres::types::Kind::Multirange(inner) => {},
	};

	if is_not_null { (ts_type, ts_hint) }
	else { (format!("{ts_type} | null"), format!("Nullable({ts_hint})")) }
}
fn columns_to_ts_type(columns: Vec<FullColumn>) -> String {
	if columns.len() == 1 {
		let column = &columns[0];
		let is_not_null = column.column_info.as_ref().map(|c| c.not_null).unwrap_or(false);
		return pg_to_ts_info(&column.pg_type, is_not_null).0.to_string();
	}

	unimplemented!()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// use sqlparser::parser::Parser;

	// let dialect = sqlparser::dialect::PostgreSqlDialect {};
	// let sql = "select :hello as yo;";
	// dbg!(Parser::parse_sql(&dialect, sql).unwrap());


	let args: Vec<String> = std::env::args().skip(1).collect();
	let admin_connection_string = args.get(0).expect("first parameter should be a database url");
	let config = admin_connection_string.parse::<postgres::Config>()?;
	let pool = deadpool::Pool::builder(deadpool::Manager::new(config.clone(), postgres::NoTls)).max_size(5).build()?;
	let client = pool.get().await?;

	use postgres_from_row::FromRow;
	let table_column_map = client.query(r#"
		select
			-- sch.nspname as schema_name,
			-- tab.relname as table_name,
			tab.oid as table_oid,
			-- col.attname as column_name,
			col.attnum as column_id,
			col.attnotnull as not_null
			-- pg_get_expr(col_detail.adbin, col_detail.adrelid) is not null as has_default
			-- pg_get_expr(col_detail.adbin, col_detail.adrelid) as default_value
		from
			pg_catalog.pg_attribute as col
			join pg_catalog.pg_class as tab on col.attrelid = tab.oid
			join pg_catalog.pg_namespace as sch on tab.relnamespace = sch.oid
			left join pg_catalog.pg_attrdef as col_detail on (col.attrelid = col_detail.adrelid and col.attnum = col_detail.adnum)
		where
			tab.relkind in ('r', 'v', 'm')
			and col.attnum > 0 -- no system columns
			and not col.attisdropped -- no dropped columns
			and sch.nspname not in ('pg_catalog', 'information_schema');
	"#, &[]).await?.iter()
		.map(ColumnInfo::from_row)
		.map(|info| ((info.table_oid, info.column_id), info))
		.collect::<std::collections::HashMap<_, _>>();

	let raw_queries_filename = args.get(1).expect("second parameter should be a json filename with raw queries in it");
	let raw_queries: Vec<RawQuery> = serde_json::from_slice(tokio::fs::read(raw_queries_filename).await?.as_slice())?;
	let mut full_statements = vec![];
	for raw_query in raw_queries {
		let statement = client.prepare(&raw_query.query).await?;

		let params = statement.params().iter()
			// TODO this is lazy for now. this will rely on the ident etc, using sqlparser
			.enumerate()
			.map(|(index, p)| FullParam{ name: format!("_{}", index), pg_type: p.to_owned() })
			.collect();

		let columns = statement.columns().iter().map(|c| {
			match (c.table_oid(), c.column_id()) {
				(Some(table_oid), Some(column_id)) => {
					let column_info = table_column_map.get(&(table_oid, column_id)).unwrap().clone();
					FullColumn{ pg_type: c.type_().clone(), column_info: Some(column_info) }
				},
				_ => FullColumn{ pg_type: c.type_().clone(), column_info: None },
			}
		}).collect();

		full_statements.push(FullStatement{ name: raw_query.name, query: raw_query.query, params, columns })
	}

	for full_statement in full_statements {
		let mut params_decl = vec![];
		let mut params_use = vec![];
		for param in full_statement.params {
			let (ts_type, ts_hint) = pg_to_ts_info(&param.pg_type, !param.name.ends_with("?"));
			params_decl.push(format!("{}: {}", param.name, ts_type));
			params_use.push(format!("[{}, {}]", param.name, ts_hint));
		}
		let params_decl = params_decl.join(", ");
		let params_use = params_use.join(", ");

		let return_decl = columns_to_ts_type(full_statement.columns);

		// TODO this will have to be based on an analysis of the query? likely more we have to support this on the runtime side,
		// just letting them say
		let execution_fn = "op_sql_fetch_all";

		let out_typescript = format!(r#"
			function {name}(runtime: Runtime, {params_decl}): Promise<{return_decl}> {{
				return runtime.{execution_fn}(`{query}`, [{params_use}])
			}}
		"#,
			name=full_statement.name,
			query=full_statement.query,
			params_decl=params_decl,
			params_use=params_use,
			return_decl=return_decl,
			execution_fn=execution_fn,
		);

		println!("{out_typescript}");
	}




	// for column in statement.columns() {
	// 	println!("for {}", column.name());
	// 	match (column.table_oid(), column.column_id()) {
	// 		(Some(table_oid), Some(column_id)) => {
	// 			let column_info = column_infos.get(&(table_oid, column_id)).unwrap();
	// 			println!("not_null={}, has_default={}", column_info.not_null, column_info.has_default);
	// 		},
	// 		_ => {
	// 			println!("do nothing");
	// 		},
	// 	}
	// }

	// let mut client = pool.get().await?;

	// #[cfg(debug_assertions)]
	// let votebase_server_password = "votebase_server_dev_pass".to_string();
	// #[cfg(not(debug_assertions))]
	// let votebase_server_password = {
	// 	use base64::Engine;
	// 	use rand::{Rng, SeedableRng};
	// 	let mut random_bytes = [0u8; 526];
	// 	let mut rng = rand::rngs::StdRng::from_os_rng();
	// 	rng.fill(&mut random_bytes);
	// 	base64::prelude::BASE64_STANDARD.encode(random_bytes)
	// };

	// let db_database = config.get_dbname().unwrap().to_owned();
	// let schema_sql = format!(include_str!("../schema.sql"), db_database=db_database, votebase_server_password=votebase_server_password);
	// client.batch_execute(&schema_sql).await?;

	// votebase_common::runtime::create_ruleset(
	// 	&config, &mut client,
	// 	None, "root",
	// 	&vec!["__insert_initial".to_string()], &vec![],
	// 	include_str!("../../rulesets/accept-any/ruleset.ts"), "",
	// ).await?;

	// println!("{votebase_server_password}");

	Ok(())
}

macro_rules! match_pg_type {
	($expr:expr, $($variant:ident => $e:expr;)*) => {
		match $expr {
			$(
				postgres::types::Type::$variant => $e,
			)*
			_ => panic!("don't know what to do with postgres type: {}", $expr),
		}
	}
}

fn base_pg_type_to_ts_info(typ: &postgres::types::Type) -> (&str, &str) {
	match_pg_type!(*typ,
		TEXT => ("string", "'string'");
		JSON => ("JsonValue", "'json'");
		JSONB => ("JsonValue", "'json'");
		BOOL => ("boolean", "'bool'");
		// BYTEA => ("bytea", "'bytea'");
		// CHAR => ("char", "'char'");
		// INT8 => ("int8", "'int8'");
		// INT2 => ("int2", "'int2'");
		// INT4 => ("int4", "'int4'");
		// FLOAT4 => ("float4", "'float4'");
		// FLOAT8 => ("float8", "'float8'");
		// NUMERIC => ("numeric", "'numeric'");
		// MONEY => ("money", "'money'");

		// VARCHAR => ("varchar", "'varchar'");
		// DATE => ("date", "'date'");
		// TIME => ("time", "'time'");
		// TIMESTAMP => ("timestamp", "'timestamp'");
		// TIMESTAMPTZ => ("timestamptz", "'timestamptz'");

		// CIDR => ("cidr", "'cidr'");
		// INET => ("inet", "'inet'");
		// TIMETZ => ("timetz", "'timetz'");
		// VOID => ("void", "'void'");

		// MACADDR8 => ("macaddr8", "'macaddr8'");
		// MACADDR => ("macaddr", "'macaddr'");
		// XML => ("xml", "'xml'");
		// UUID => ("uuid", "'uuid'");

		// INT2_VECTOR => ("int2_vector", "'int2_vector'");
		// REGPROC => ("regproc", "'regproc'");
		// NAME => ("name", "'name'");
		// OID => ("oid", "'oid'");
		// TID => ("tid", "'tid'");
		// XID => ("xid", "'xid'");
		// CID => ("cid", "'cid'");
		// OID_VECTOR => ("oid_vector", "'oid_vector'");
		// PG_DDL_COMMAND => ("pg_ddl_command", "'pg_ddl_command'");
		// PG_NODE_TREE => ("pg_node_tree", "'pg_node_tree'");
		// TABLE_AM_HANDLER => ("table_am_handler", "'table_am_handler'");
		// INDEX_AM_HANDLER => ("index_am_handler", "'index_am_handler'");
		// POINT => ("point", "'point'");
		// LSEG => ("lseg", "'lseg'");
		// PATH => ("path", "'path'");
		// BOX => ("box", "'box'");
		// POLYGON => ("polygon", "'polygon'");
		// LINE => ("line", "'line'");
		// UNKNOWN => ("unknown", "'unknown'");
		// CIRCLE => ("circle", "'circle'");
		// ACLITEM => ("aclitem", "'aclitem'");
		// BPCHAR => ("bpchar", "'bpchar'");
		// INTERVAL => ("interval", "'interval'");
		// BIT => ("bit", "'bit'");
		// VARBIT => ("varbit", "'varbit'");
		// REFCURSOR => ("refcursor", "'refcursor'");
		// REGPROCEDURE => ("regprocedure", "'regprocedure'");
		// REGOPER => ("regoper", "'regoper'");
		// REGOPERATOR => ("regoperator", "'regoperator'");
		// REGCLASS => ("regclass", "'regclass'");
		// REGTYPE => ("regtype", "'regtype'");
		// RECORD => ("record", "'record'");
		// CSTRING => ("cstring", "'cstring'");
		// ANY => ("any", "'any'");
		// ANYARRAY => ("anyarray", "'anyarray'");
		// TRIGGER => ("trigger", "'trigger'");
		// LANGUAGE_HANDLER => ("language_handler", "'language_handler'");
		// INTERNAL => ("internal", "'internal'");
		// ANYELEMENT => ("anyelement", "'anyelement'");
		// ANYNONARRAY => ("anynonarray", "'anynonarray'");
		// TXID_SNAPSHOT => ("txid_snapshot", "'txid_snapshot'");
		// FDW_HANDLER => ("fdw_handler", "'fdw_handler'");
		// PG_LSN => ("pg_lsn", "'pg_lsn'");
		// TSM_HANDLER => ("tsm_handler", "'tsm_handler'");
		// PG_NDISTINCT => ("pg_ndistinct", "'pg_ndistinct'");
		// PG_DEPENDENCIES => ("pg_dependencies", "'pg_dependencies'");
		// ANYENUM => ("anyenum", "'anyenum'");
		// TS_VECTOR => ("ts_vector", "'ts_vector'");
		// TSQUERY => ("tsquery", "'tsquery'");
		// GTS_VECTOR => ("gts_vector", "'gts_vector'");
		// REGCONFIG => ("regconfig", "'regconfig'");
		// REGDICTIONARY => ("regdictionary", "'regdictionary'");
		// ANY_RANGE => ("any_range", "'any_range'");
		// EVENT_TRIGGER => ("event_trigger", "'event_trigger'");
		// INT4_RANGE => ("int4_range", "'int4_range'");
		// NUM_RANGE => ("num_range", "'num_range'");
		// TS_RANGE => ("ts_range", "'ts_range'");
		// TSTZ_RANGE => ("tstz_range", "'tstz_range'");
		// DATE_RANGE => ("date_range", "'date_range'");
		// INT8_RANGE => ("int8_range", "'int8_range'");
		// JSONPATH => ("jsonpath", "'jsonpath'");
		// REGNAMESPACE => ("regnamespace", "'regnamespace'");
		// REGROLE => ("regrole", "'regrole'");
		// REGCOLLATION => ("regcollation", "'regcollation'");
		// INT4MULTI_RANGE => ("int4multi_range", "'int4multi_range'");
		// NUMMULTI_RANGE => ("nummulti_range", "'nummulti_range'");
		// TSMULTI_RANGE => ("tsmulti_range", "'tsmulti_range'");
		// TSTZMULTI_RANGE => ("tstzmulti_range", "'tstzmulti_range'");
		// DATEMULTI_RANGE => ("datemulti_range", "'datemulti_range'");
		// INT8MULTI_RANGE => ("int8multi_range", "'int8multi_range'");
		// ANYMULTI_RANGE => ("anymulti_range", "'anymulti_range'");
		// ANYCOMPATIBLEMULTI_RANGE => ("anycompatiblemulti_range", "'anycompatiblemulti_range'");
		// PG_BRIN_BLOOM_SUMMARY => ("pg_brin_bloom_summary", "'pg_brin_bloom_summary'");
		// PG_BRIN_MINMAX_MULTI_SUMMARY => ("pg_brin_minmax_multi_summary", "'pg_brin_minmax_multi_summary'");
		// PG_MCV_LIST => ("pg_mcv_list", "'pg_mcv_list'");
		// PG_SNAPSHOT => ("pg_snapshot", "'pg_snapshot'");
		// XID8 => ("xid8", "'xid8'");
		// ANYCOMPATIBLE => ("anycompatible", "'anycompatible'");
		// ANYCOMPATIBLEARRAY => ("anycompatiblearray", "'anycompatiblearray'");
		// ANYCOMPATIBLENONARRAY => ("anycompatiblenonarray", "'anycompatiblenonarray'");
		// ANYCOMPATIBLE_RANGE => ("anycompatible_range", "'anycompatible_range'");
	)
}


// use sqlparser::ast::{Expr, VisitMut, VisitorMut};
// use sqlparser::parser::Parser;
// use std::collections::HashMap;

// /// A visitor that transforms named parameters (`:param_name`) to positional parameters (`$1`)
// struct NamedParamReplacer {
// 	param_map: HashMap<String, usize>,
// 	next_position: usize,
// }

// impl NamedParamReplacer {
// 	fn new() -> Self {
// 		Self {
// 			param_map: HashMap::new(),
// 			next_position: 1,
// 		}
// 	}

// 	fn get_position(&mut self, param_name: &str) -> usize {
// 		*self.param_map.entry(param_name.to_string()).or_insert_with(|| {
// 			let pos = self.next_position;
// 			self.next_position += 1;
// 			pos
// 		})
// 	}
// }

// // have to figure out what's really going on here
// // https://docs.rs/sqlparser/latest/sqlparser/ast/trait.VisitMut.html
// // https://docs.rs/sqlparser/latest/sqlparser/ast/trait.VisitorMut.html
// impl VisitorMut for NamedParamReplacer {
// 	type Break = ();

// 	fn post_visit_expr(&mut self, expr: &mut Expr) -> std::ops::ControlFlow<Self::Break> {
// 		match expr {
// 			// I think it's always going to be Value
// 			Expr::Value(sqlparser::ast::ValueWithSpan{ value: sqlparser::ast::Value::Placeholder(placeholder), .. }) => {
// 				if let Some(param_name) = placeholder.strip_prefix(':') {
// 					let position = self.get_position(param_name);
// 					*placeholder = format!("${}", position);
// 				}
// 			},

// 			// Expr::Identifier(ident) => {
// 			// 	if let Some(param_name) = ident.value.strip_prefix(':') {
// 			// 		let position = self.get_position(param_name);
// 			// 		*expr = Expr::Value(Value::Placeholder(format!("${}", position)));
// 			// 	}
// 			// },

// 			_ => {}
// 		}

// 		// Continue traversal to visit all child expressions
// 		// sqlparser::ast::visit_expr_mut(self, expr);
// 		std::ops::ControlFlow::Continue(())
// 	}
// }

// pub fn convert_named_params_to_positional(sql: &str) -> Result<(String, HashMap<String, usize>), String> {
// 	let mut stmts = Parser::parse_sql(&sqlparser::dialect::PostgreSqlDialect{}, sql)
// 		.map_err(|e| format!("Failed to parse SQL: {}", e))?;

// 	let mut replacer = NamedParamReplacer::new();
// 	stmts.visit(&mut replacer);

// 	let modified_sql = stmts.iter()
// 		.map(|stmt| stmt.to_string())
// 		.collect::<Vec<_>>()
// 		.join("; ");

// 	Ok((modified_sql, replacer.param_map))
// }
