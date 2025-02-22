-- # when a new ruleset is created:
-- - a schema is created with name `"ruleset:{full_path}"`
-- - a role is created for each `fn_type` to be used by that `"role:{full_path}|{fn_type}"`
-- 	- I actually think these two have to have a cryptographically secure name, which we have to store in the database to remember
-- 	- we need to ensure that the view role can't modify data, and the action role can't modify database objects.
-- 	- the dangerous part of all of this is the `db_migration` we allow. we need a safe role that is given ddl abilities
-- 	- also I think the only way to prevent the creation of `security definer` functions by the migrator role is to check the string to see if it includes them!
-- 	- the really tricky part is how to think about *child* privileges. it totally makes sense to allow a parent ruleset to define functions that allow its children
-- - I think in general we need to assume good intent here? is this even possible?
-- - we can always safely ensure that view roles can't modify the database even in the presence of `security definer` functions if views can only read tables and read "views" (the postgres kind). this means modification is only possible through modifiable views, which have to be used in an explicitly modifying way. then we can just allow security definer functions using a migrator role
-- - I think this means we also need to create a role `"role:{full_path}|migrator"`
-- - within a ruleset, permissions are managed by the ruleset itself, and the member id system (using exclusively magic links), because it's just too insane and complicated to allow them to create roles
-- - in all above strings, `|` is included

-- views
-- grant connection to database
grant connect on database dev_db to "role:full_path|view";
-- grant schema usage
grant usage on schema "ruleset:{full_path}" to "role:full_path|view";

-- grant read access to future tables and views
alter default privileges in schema target_schema
grant select on tables to "role:full_path|view";

-- actions
-- grant connection to database
grant connect on database dev_db to "role:full_path|action";
-- grant schema usage
grant usage on schema "ruleset:{full_path}" to "role:full_path|action";

-- grant permissions for future tables
alter default privileges in schema "ruleset:{full_path}"
grant select, insert, update, delete on tables to "role:full_path|action";

-- grant for future sequences
alter default privileges in schema "ruleset:{full_path}"
grant usage on sequences to "role:full_path|action";

alter default privileges in schema "ruleset:{full_path}"
grant execute on functions to "role:full_path|action";

-- migrator
-- grant connection to database
grant connect on database dev_db to "role:full_path|migrator";

-- grant schema creation and usage
grant create, usage on schema "ruleset:{full_path}" to "role:full_path|migrator";

-- grant table management permissions
alter default privileges in schema "ruleset:{full_path}"
grant all privileges on tables to "role:full_path|migrator";

-- grant sequence management
alter default privileges in schema "ruleset:{full_path}"
grant all privileges on sequences to "role:full_path|migrator";

-- grant function management
alter default privileges in schema "ruleset:{full_path}"
grant all privileges on functions to "role:full_path|migrator";

-- grant type management
alter default privileges in schema "ruleset:{full_path}"
grant all privileges on types to "role:full_path|migrator";




-- STABLE and IMMUTABLE Functions
-- STABLE and IMMUTABLE functions are guaranteed not to modify the database. STABLE functions return consistent results within a single table scan, while IMMUTABLE functions always return the same result for the same inputs.


-- -- Grant execute permission on all current functions
-- GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO role_name;

-- -- Set default privileges for future functions
-- ALTER DEFAULT PRIVILEGES IN SCHEMA public
-- GRANT EXECUTE ON FUNCTIONS TO role_name;


-- Important Security Notes
-- Even with EXECUTE privileges, functions will still run with the permissions of the executing role, not the function owner. This means the role will need appropriate underlying permissions to access any data the functions read.



-- -- Revoke function creation permission
-- REVOKE CREATE ON SCHEMA public FROM role_name;

-- -- If needed, grant regular function creation while preventing SECURITY DEFINER
-- GRANT CREATE ON SCHEMA public TO role_name;
-- ALTER DEFAULT PRIVILEGES FOR ROLE role_name
-- 		REVOKE ALL ON FUNCTIONS FROM PUBLIC;





