--! create_detached_recurring_action
insert into votebase_catalog.detached_recurring_action
	(full_path, description, "start", recurrence_granularity, recurrence_multiplier, action_name, action_arg)
values (:full_path, :description, :start, :recurrence_granularity, :recurrence_multiplier, :action_name, :action_arg)
returning id, next_scheduled_time;


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


--! remove_detached_recurring_action
delete from votebase_catalog.detached_recurring_action
where id = :scheduled_action_uuid;



--! create_detached_scheduled_action
insert into votebase_catalog.detached_scheduled_action (full_path, description, scheduled_time, action_name, action_arg)
values (:full_path, :description, :scheduled_time, :action_name, :action_arg)
returning id;


--! acquire_detached_scheduled_action
with updated as (
	update votebase_catalog.detached_scheduled_action
	set executing = true
	where executing = false and id = :scheduled_action_uuid
	returning description, scheduled_time, full_path, action_name, action_arg as arg
)
select code, action_pass, migrator_pass, updated.*
from updated inner join votebase_catalog.ruleset as r on updated.full_path = r.full_path;

-- release?

--! remove_detached_scheduled_action
delete from votebase_catalog.detached_scheduled_action
where id = :scheduled_action_uuid;

--! select_slim_detached_scheduled_actions
select id, scheduled_time
from votebase_catalog.detached_scheduled_action;

--! select_slim_detached_recurring_actions
select id, next_scheduled_time
from votebase_catalog.detached_recurring_action
where not executing;

--! test_select_detached_scheduled_actions
select id, description, scheduled_time, full_path, action_name, action_arg
from votebase_catalog.detached_scheduled_action;
