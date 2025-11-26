insert into fruit (name, inserted_by_member_id)
values (":fruit_name", ":inserted_by_member_id")
returning id;
