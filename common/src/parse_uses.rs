fn construct_abstract_use_standin(use_kind: &UseKind) -> String {
	let dummy_name = "TODO".to_string();

	match use_kind {
		UseKind::Table { columns, .. } => {
			let columns_str = columns.iter().map(|UseColumn { name, pg_type, can_null }| {
				let null_portion = if *can_null { " not null" } else { "" };
				format!("{name} {pg_type}{null_portion}")
			}).collect::<Vec<_>>().join(", ");

			format!("create table {dummy_name} ({columns_str});")
		},
		UseKind::Function { is_action, params, return_type } => {
			let volatility = if *is_action { "volatile" } else { "stable" };
			let params_str = params.into_iter().enumerate()
				.map(|(idx, pg_type)| format!("_{idx} {pg_type}"))
				.collect::<Vec<_>>().join(", ");
			format!("create function {dummy_name}({params_str}) returns {return_type} as $$ begin raise exception ''; end; $$ language plpgsql {volatility};")
		},
	}
}


// I don't actually think it's all that possible to have "blank placeholder types", especially when the structure of postgres types is so rigid in regards to how you actually construct them. if you want to do this, it needs to be similar to the table system in the sense that the actual *structure* of the type is what you have to declare you expect (composite, enumerated, range, base?)
// https://www.postgresql.org/docs/current/sql-createtype.html

// the *abstract* system will be structured roughly like this:
// "~v1": "ReferenceTable(c1 t1, c2 t2?)" // https://docs.rs/sqlparser/latest/sqlparser/ast/struct.TableAlias.html
// "~v2": "QueryAndReferenceTable(c1 t1?, c2 t2)"
// "~v3": "CallViewFunction(t1, t2) -> tr"
// "~v4": "CallActionFunction(t1, t2) -> tr"

// and the concrete fulfillments
// "~v1": "ruleset_path.table_name(c1, c2)"
// "~v2": "ruleset_path.table_name(c1, c2)"
// "~v3": "ruleset_path.function_name"
// "~v4": "ruleset_path.function_name"
// https://www.postgresql.org/docs/current/catalog-pg-proc.html



// use sqlparser::dialect::PostgreSqlDialect;
// use sqlparser::parser::Parser;
// use sqlparser::ast::{Statement, CreateFunction, ObjectName, OperateFunctionArg, DataType};

// #[derive(Debug)]
// struct PgSignature {
// 	name: ObjectName,
// 	args: Vec<OperateFunctionArg>,
// 	return_type: Option<DataType>,
// }

// fn parse_postgres_signature(signature: &str) -> Result<PgSignature, String> {
// 	let sql_wrapper = format!("CREATE FUNCTION {signature} AS 'x' LANGUAGE sql");

// 	let dialect = PostgreSqlDialect {};
// 	let ast = Parser::parse_sql(&dialect, &sql_wrapper)
// 		.map_err(|e| e.to_string())?;

// 	if let Some(Statement::CreateFunction { name, args, return_type, .. }) = ast.into_iter().next() {
// 		Ok(PgSignature {
// 			name,
// 			args: args.unwrap_or_default(),
// 			return_type,
// 		})
// 	} else {
// 		Err("Failed to parse function signature: invalid syntax or not a function definition".to_string())
// 	}
// }

// fn main() {
// 	let raw_sig = "myschema.process_data(id integer, data text) RETURNS boolean";

// 	match parse_postgres_signature(raw_sig) {
// 		Ok(parsed) => {
// 			println!("Function Name: {}", parsed.name); // e.g., myschema.process_data

// 			println!("Parameters:");
// 			for arg in parsed.args {
// 				// arguments often contain names and types
// 				println!("  - {:?}", arg);
// 			}

// 			if let Some(rt) = parsed.return_type {
// 				println!("Return Type: {}", rt); // e.g., BOOLEAN
// 			}
// 		}
// 		Err(e) => eprintln!("Error: {}", e),
// 	}
// }


// https://docs.rs/sqlparser/latest/sqlparser/ast/struct.TableAlias.html
