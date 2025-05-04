--! enroll_member
insert into votebase_catalog.member (email) values (:email) returning id;


--! remove_member_by_email
delete from votebase_catalog.member where email = :email;


--! remove_member_by_uuid
delete from votebase_catalog.member where id = :member_uuid;


--! test_delete_all_members
delete from votebase_catalog.member where true;

--! test_select_all_members
select id, email from votebase_catalog.member;
