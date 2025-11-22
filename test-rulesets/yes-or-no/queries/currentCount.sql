select
	count(case when is_yes then 1 else null end) as yes_count,
	count(case when not is_yes then 1 else null end) as no_count
from vote;
