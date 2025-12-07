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
