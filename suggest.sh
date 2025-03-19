# cat << EOF
# {
#   "code": $(jq -R -s '.' < rulesets/three-yes/ruleset.ts),
#   "db_schema": $(jq -R -s '.' < rulesets/three-yes/schema.sql),
#   "db_migration": ""
# }
# EOF


PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost dev_db -c 'select full_path, actions, views from votebase_catalog.ruleset' | cat
PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost dev_db -c 'select * from votebase_catalog.candidate_replacement_ruleset' | cat

# code, db_schema
# migrator_pass, action_pass, view_pass

curl -v -H "Content-Type: application/json" --data @- 'http://localhost:8080/fn/action/root|__insert_initial' << EOF
{
  "code": $(jq -R -s '.' < rulesets/three-yes/ruleset.ts),
  "db_schema": $(jq -R -s '.' < rulesets/three-yes/schema.sql),
  "db_migration": ""
}
EOF

PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost dev_db -c 'select full_path, actions, views from votebase_catalog.ruleset' | cat
PGPASSWORD='dev_admin_password' psql -U dev_admin_user -h localhost dev_db -c 'select * from votebase_catalog.candidate_replacement_ruleset' | cat
