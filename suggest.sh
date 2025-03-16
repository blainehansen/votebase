# cat << EOF
# {
#   "code": $(jq -R -s '.' < rulesets/three-yes/ruleset.ts),
#   "db_schema": $(jq -R -s '.' < rulesets/three-yes/schema.sql),
#   "db_migration": ""
# }
# EOF


curl -X POST -H "Content-Type: application/json" --data @- 'http://localhost:8080/fn/action/root|__insert_initial' << EOF
{
  "code": $(jq -R -s '.' < rulesets/three-yes/ruleset.ts),
  "db_schema": $(jq -R -s '.' < rulesets/three-yes/schema.sql),
  "db_migration": ""
}
EOF
