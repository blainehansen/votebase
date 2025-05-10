use votebase_queries::{tokio_postgres as postgres, deadpool_postgres as deadpool};

type AnyError = Box<dyn std::error::Error>;

#[derive(Debug)]
struct FullColumn {
	pg_type: postgres::types::Type,
	column_info: Option<ColumnInfo>,
}

#[derive(Debug)]
struct FullParam {
	name: String,
	index: usize,
	null_allowed: bool,
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

async fn read_sql_file(path: std::path::PathBuf) -> Result<RawQuery, tokio::io::Error> {
	let query = tokio::fs::read_to_string(&path).await?;
	let name = path.to_string_lossy().to_string();
	Ok(RawQuery { name, query })
}

type TableColumnMap = HashMap<(u32, i16), ColumnInfo>;
async fn prepare_query(
	client: &deadpool::Client,
	table_column_map: &TableColumnMap,
	raw_query: RawQuery,
) -> Result<FullStatement, postgres::Error> {
	let (actual_sql, query_vars) = convert_named_params_to_positional(&raw_query.query).unwrap();

	let statement = client.prepare(&actual_sql).await?;

	let params = statement.params().iter()
		.enumerate()
		.map(|(index, p)| {
			let param_name = query_vars.get(&index).unwrap();
			let null_allowed = param_name.ends_with("?");
			let param_name = param_name.strip_suffix("?").unwrap_or(param_name);

			FullParam{ name: param_name.to_string(), index, null_allowed, pg_type: p.to_owned() }
		})
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

	Ok(FullStatement{ name: raw_query.name, query: actual_sql, params, columns })
}


#[tokio::main]
async fn main() -> Result<(), AnyError> {
	let args: Vec<String> = std::env::args().skip(1).collect();
	let admin_connection_string = args.get(0).expect("first parameter should be a database url");
	let config = admin_connection_string.parse::<postgres::Config>()?;
	let pool = deadpool::Pool::builder(deadpool::Manager::new(config.clone(), postgres::NoTls)).max_size(5).build()?;
	let client = pool.get().await?;
	let queries_dir = args.get(1).expect("second parameter should be a directory").to_string();

	let sql_files = tokio::task::spawn_blocking(move || {
		walkdir::WalkDir::new(queries_dir).follow_links(false).into_iter()
			.filter_map(|e| e.ok())
			.filter(|e| e.file_type().is_file() && e.file_name().to_string_lossy().ends_with(".sql"))
			.map(|e| e.into_path())
			.collect::<Vec<_>>()
	}).await?;

	use postgres_from_row::FromRow;
	let table_column_map: TableColumnMap = client.query(r#"
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

	let full_statements = futures::future::try_join_all(
		sql_files.into_iter().map(async |path| {
			let raw_query = read_sql_file(path).await?;
			let full_statement = prepare_query(&client, &table_column_map, raw_query).await?;
			Ok::<_, AnyError>(full_statement)
		})
	).await?;

	for mut full_statement in full_statements {
		let mut params_decl = vec![];
		let mut params_use = vec![];
		full_statement.params.sort_by_key(|p| p.index);
		for param in full_statement.params {
			let (ts_type, ts_hint) = pg_to_ts_info(&param.pg_type, !param.null_allowed);
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


use sqlparser::ast::{VisitMut, VisitorMut};
use std::collections::HashMap;

struct NamedParamReplacer {
	param_map: HashMap<usize, String>,
	next_position: usize,
}

impl NamedParamReplacer {
	fn new() -> Self {
		Self { param_map: HashMap::new(), next_position: 0 }
	}

	fn get_position(&mut self, param_name: &str) -> usize {
		let position = self.next_position;
		self.param_map.entry(position).or_insert_with(|| {
			self.next_position += 1;
			param_name.to_string()
		});
		self.next_position
	}
}

impl VisitorMut for NamedParamReplacer {
	type Break = ();

	fn post_visit_expr(&mut self, expr: &mut sqlparser::ast::Expr) -> std::ops::ControlFlow<Self::Break> {
		use sqlparser::{ast::{self, Expr}};

		match expr {
			Expr::Value(ast::ValueWithSpan{ value: ast::Value::Placeholder(placeholder), .. }) => {
				if let Some(param_name) = placeholder.strip_prefix(':') {
					let position = self.get_position(param_name);
					*placeholder = format!("${}", position);
				}
			},

			Expr::Identifier(ident) => {
				if let Some(param_name) = ident.value.strip_prefix(':') {
					let position = self.get_position(param_name);
					*expr = Expr::Value(ast::ValueWithSpan{
						value: ast::Value::Placeholder(format!("${}", position)),
						span: ident.span,
					});
				}
			},

			_ => {}
		}

		std::ops::ControlFlow::Continue(())
	}
}

fn convert_named_params_to_positional(sql: &str) -> Result<(String, HashMap<usize, String>), String> {
	let mut stmts = sqlparser::parser::Parser::parse_sql(&sqlparser::dialect::PostgreSqlDialect{}, sql)
		.map_err(|e| format!("Failed to parse SQL: {}", e))?;

	let mut replacer = NamedParamReplacer::new();
	stmts.visit(&mut replacer);

	let modified_sql = stmts.iter()
		.map(|stmt| stmt.to_string())
		.collect::<Vec<_>>()
		.join("; ");

	Ok((modified_sql, replacer.param_map))
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
