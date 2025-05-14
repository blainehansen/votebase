use votebase_queries::{tokio_postgres as postgres, deadpool_postgres as deadpool};
use std::collections::HashMap;

type AnyError = Box<dyn std::error::Error>;

#[derive(Debug)]
struct FullColumn {
	name: String,
	pg_type: postgres::types::Type,
	column_info: Option<ColumnInfo>,
}

#[derive(Debug)]
struct FullParam {
	// name: String,
	index: usize,
	null_allowed: bool,
	pg_type: postgres::types::Type,
}

#[derive(Debug)]
enum StatementKind {
	Query,
	Statement,
	Statements
}

#[derive(Debug)]
struct FullStatement {
	name: String,
	query: String,
	statement_kind: StatementKind,
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


pub async fn generate_queries(queries_dir: std::path::PathBuf, client: &deadpool::Client) -> Result<String, AnyError> {
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
		.collect::<HashMap<_, _>>();

	let full_statements = futures::future::try_join_all(
		sql_files.into_iter().map(async |path| {
			let raw_query = read_sql_file(path).await?;
			let full_statement = prepare_query(&client, &table_column_map, raw_query).await?;
			Ok::<_, AnyError>(full_statement)
		})
	).await?;

	let generated_fields = full_statements.into_iter().map(|mut full_statement| {
		full_statement.params.sort_by_key(|p| p.index);
		let hints = full_statement.params.into_iter()
			.map(|param| pg_type_to_ts_hint(&param.pg_type, !param.null_allowed))
			.collect::<Vec<_>>()
			.join(", ");

		// TODO ought to assert no backticks are present in the query

		let ret = columns_to_type_hint(full_statement.columns);
		let construction = match full_statement.statement_kind {
			StatementKind::Query => { format!(
				"new votebase._.QueryExecutor(`{query}`, [{hints}], {ret})",
				query=full_statement.query,
				hints=hints,
				ret=ret,
			) },
			StatementKind::Statement => { format!(
				"votebase._.StatementExecutor(`{query}`, [{hints}])",
				query=full_statement.query,
				hints=hints,
			) },
			StatementKind::Statements => { format!(
				"votebase._.StatementsExecutor(`{query}`)",
				query=full_statement.query,
			) },
		};

		format!(
			r#"	"{name}": {construction},"#,
			name=full_statement.name,
			construction=construction,
		)
	}).collect::<Vec<_>>().join("\n");

	Ok(generated_fields)
}

async fn read_sql_file(path: std::path::PathBuf) -> Result<RawQuery, tokio::io::Error> {
	let query = tokio::fs::read_to_string(&path).await?;
	let name = path.to_string_lossy().to_string();
	Ok(RawQuery { name, query })
}


fn pg_type_to_ts_hint(typ: &postgres::types::Type, is_not_null: bool) -> String {
	let ts_hint = match typ.kind() {
		postgres::types::Kind::Simple | postgres::types::Kind::Pseudo => {
			let ts_hint = crate::gen_ts::base_pg_type_to_ts_info(typ);
			ts_hint.to_string()
		},
		postgres::types::Kind::Enum(_) => {
			"'string'".into()
		},
		postgres::types::Kind::Array(inner) => {
			let ts_hint = crate::gen_ts::base_pg_type_to_ts_info(inner);
			ts_hint.to_string()
		},
		postgres::types::Kind::Domain(inner) => {
			let ts_hint = crate::gen_ts::base_pg_type_to_ts_info(inner);
			ts_hint.to_string()
		},
		postgres::types::Kind::Composite(fields) => {
			let mut ts_hints = vec![];
			for field in fields {
				let field_name = field.name();
				let ts_hint = pg_type_to_ts_hint(field.type_(), false);
				ts_hints.push(format!("{field_name}: {ts_hint}"));
			}
			format!("{{ {} }}", ts_hints.join(", "))
		},
		_ => panic!("don't know what to do with pg type: {}", typ),
		// postgres::types::Kind::Range(inner) => {},
		// postgres::types::Kind::Multirange(inner) => {},
	};

	if is_not_null { ts_hint }
	else { format!("Nullable({ts_hint})") }
}
fn columns_to_type_hint(columns: Vec<FullColumn>) -> String {
	if columns.len() == 1 {
		let column = &columns[0];
		let is_not_null = column.column_info.as_ref().map(|c| c.not_null).unwrap_or(false);
		return pg_type_to_ts_hint(&column.pg_type, is_not_null).to_string();
	}

	let hints = columns.into_iter().map(|column| {
		let is_not_null = column.column_info.as_ref().map(|c| c.not_null).unwrap_or(false);
		let hint = pg_type_to_ts_hint(&column.pg_type, is_not_null);
		let name = column.name;
		format!(r#"[`{name}`, {hint}]"#)
	}).collect::<Vec<_>>().join(", ");

	format!("[{hints}]")
}


type TableColumnMap = HashMap<(u32, i16), ColumnInfo>;
async fn prepare_query(
	client: &deadpool::Client,
	table_column_map: &TableColumnMap,
	raw_query: RawQuery,
) -> Result<FullStatement, postgres::Error> {
	let (actual_sql, query_vars, stmts) = convert_named_params_to_positional(&raw_query.query).unwrap();

	let has_multiple = stmts.len() > 1;

	let (params, columns) = if has_multiple {
		for actual_sql in stmts {
			let statement = client.prepare(&actual_sql).await?;
			let params = statement.params();
			if params.len() != 0 {
				panic!("statement in multi-statement thing shouldn't have params: {:?}", params);
			}
		}

		(vec![], vec![])
	}
	else {
		let statement = client.prepare(&actual_sql).await?;

		let params = statement.params().iter()
			.enumerate()
			.map(|(index, p)| {
				let param_name = query_vars.get(&index).unwrap();
				let null_allowed = param_name.ends_with("?");
				// let param_name = param_name.strip_suffix("?").unwrap_or(param_name);

				FullParam{ /*name: param_name.to_string(),*/ index, null_allowed, pg_type: p.to_owned() }
			})
			.collect();

		let columns = statement.columns().iter().map(|c| {
			match (c.table_oid(), c.column_id()) {
				(Some(table_oid), Some(column_id)) => {
					let column_info = table_column_map.get(&(table_oid, column_id)).unwrap().clone();
					FullColumn{ name: c.name().to_string(), pg_type: c.type_().clone(), column_info: Some(column_info) }
				},
				_ => FullColumn{ name: c.name().to_string(), pg_type: c.type_().clone(), column_info: None },
			}
		}).collect::<Vec<_>>();

		(params, columns)
	};

	let statement_kind =
		if has_multiple { StatementKind::Statements }
		else if columns.len() == 0 { StatementKind::Statement}
		else { StatementKind::Query };

	Ok(FullStatement{ name: raw_query.name, query: actual_sql, statement_kind, params, columns })
}


use sqlparser::ast::{VisitMut, VisitorMut};

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

fn convert_named_params_to_positional(sql: &str) -> Result<(String, HashMap<usize, String>, Vec<String>), String> {
	let mut stmts = sqlparser::parser::Parser::parse_sql(&sqlparser::dialect::PostgreSqlDialect{}, sql)
		.map_err(|e| format!("Failed to parse SQL: {}", e))?;

	let mut replacer = NamedParamReplacer::new();
	stmts.visit(&mut replacer);

	let stmts = stmts.into_iter()
		.map(|stmt| stmt.to_string())
		.collect::<Vec<_>>();

	Ok((stmts.join("; "), replacer.param_map, stmts))
}
