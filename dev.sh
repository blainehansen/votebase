# PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost postgres -c '\l' | cat

cargo run -p votebase_cli -- \
	--db-url "postgres://dev_admin_user:dev_admin_password@localhost:5432/dev_db" \
	--ruleset-dir rulesets/three-yes-proposals \
	dev
