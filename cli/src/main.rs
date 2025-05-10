use votebase_queries::{tokio_postgres as postgres, deadpool_postgres as deadpool};

type AnyError = Box<dyn std::error::Error>;

#[tokio::main]
async fn main() -> Result<(), AnyError> {
	let args: Vec<String> = std::env::args().skip(1).collect();
	let connection_string = args.get(0).expect("first parameter should be a database url");
	let config = connection_string.parse::<postgres::Config>()?;
	let pool = deadpool::Pool::builder(deadpool::Manager::new(config.clone(), postgres::NoTls)).max_size(5).build()?;
	let client = pool.get().await?;
	let queries_dir = args.get(1).expect("second parameter should be a directory").to_string();

	let generated = votebase_common::gen_queries::generate_queries(queries_dir.clone(), &client).await?;

	let mut file = tokio::fs::OpenOptions::new().write(true).create(true)
		.open(format!("{queries_dir}.ts")).await?;

	use tokio::io::AsyncWriteExt;
	file.write_all(generated.as_bytes()).await?;

	Ok(())
}
