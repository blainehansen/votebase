--! select_slim_detached_scheduled_action
select id, scheduled_time
from votebase_catalog.detached_scheduled_action;


--! acquire_detached_recurring_action
with updated as (
	update votebase_catalog.detached_recurring_action
	set executing = true
	where executing = false and id = :recurring_action_id
	returning description, next_scheduled_time, full_path, action_name, action_arg as arg
)
select code, action_pass, migrator_pass, updated.*
from updated inner join votebase_catalog.ruleset as r on updated.full_path = r.full_path;


--! release_detached_recurring_action
update votebase_catalog.detached_recurring_action
set executing = false, executed_count = executed_count + 1
where id = :recurring_action_id
returning next_scheduled_time;
