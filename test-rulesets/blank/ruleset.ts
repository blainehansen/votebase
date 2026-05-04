// votebase.Action("propose", async () => {
// 	const u = await votebase.proposeSelfReplacement({
// 		ts_code: `
// 			votebase.Action("action1", () => {});
// 			votebase.Action("action2", () => {});
// 			votebase.View("view1", () => "yo");
// 		`,
// 		db_schema: "create table stuff (id uuid primary key, color text not null);",
// 		db_migration: "alter table stuff add column color text not null;",
// 	})
// 	if (typeof u !== 'string' || u.length !== 36)
// 		throw new Error(`proposeSelfReplacement didn't return uuid: ${u}`)
// })

// votebase.Action("replace", async () => {
// 	return { replace_self_with_uuid: "", delete_other_candidates: true }
// })

// votebase.Action("action1", () => {});
// votebase.Action("action2", () => {});
// votebase.View("view1", () => "yo");
