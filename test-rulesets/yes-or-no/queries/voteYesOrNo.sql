insert into vote(member_id, is_yes, message) values (:member_id, :is_yes, ":message?")
on conflict (member_id) do update
set is_yes = excluded.is_yes, message = excluded.message;
