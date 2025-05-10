use votebase_queries::{tokio_postgres as postgres};

macro_rules! match_pg_type {
	($expr:expr, $($variant:ident => $e:expr;)*) => {
		match $expr {
			$(
				postgres::types::Type::$variant => $e,
			)*
			_ => panic!("don't know what to do with postgres type: {}", $expr),
		}
	}
}

pub fn base_pg_type_to_ts_info(typ: &postgres::types::Type) -> (&str, &str) {
	match_pg_type!(*typ,
		TEXT => ("string", "'string'");
		VARCHAR => ("string", "'string'");
		JSON => ("JsonValue", "'json'");
		JSONB => ("JsonValue", "'json'");
		BOOL => ("boolean", "'bool'");
		INT8 => ("number", "'number'");
		INT2 => ("number", "'number'");
		INT4 => ("number", "'number'");
		NUMERIC => ("number", "'number'");
		MONEY => ("number", "'number'");

		// BYTEA => ("bytea", "'bytea'");
		// CHAR => ("char", "'char'");
		// FLOAT4 => ("float4", "'float4'");
		// FLOAT8 => ("float8", "'float8'");

		// DATE => ("date", "'date'");
		// TIME => ("time", "'time'");
		// TIMESTAMP => ("timestamp", "'timestamp'");
		// TIMESTAMPTZ => ("timestamptz", "'timestamptz'");

		// CIDR => ("cidr", "'cidr'");
		// INET => ("inet", "'inet'");
		// TIMETZ => ("timetz", "'timetz'");
		// VOID => ("void", "'void'");

		// MACADDR8 => ("macaddr8", "'macaddr8'");
		// MACADDR => ("macaddr", "'macaddr'");
		// XML => ("xml", "'xml'");
		// UUID => ("uuid", "'uuid'");

		// INT2_VECTOR => ("int2_vector", "'int2_vector'");
		// REGPROC => ("regproc", "'regproc'");
		// NAME => ("name", "'name'");
		// OID => ("oid", "'oid'");
		// TID => ("tid", "'tid'");
		// XID => ("xid", "'xid'");
		// CID => ("cid", "'cid'");
		// OID_VECTOR => ("oid_vector", "'oid_vector'");
		// PG_DDL_COMMAND => ("pg_ddl_command", "'pg_ddl_command'");
		// PG_NODE_TREE => ("pg_node_tree", "'pg_node_tree'");
		// TABLE_AM_HANDLER => ("table_am_handler", "'table_am_handler'");
		// INDEX_AM_HANDLER => ("index_am_handler", "'index_am_handler'");
		// POINT => ("point", "'point'");
		// LSEG => ("lseg", "'lseg'");
		// PATH => ("path", "'path'");
		// BOX => ("box", "'box'");
		// POLYGON => ("polygon", "'polygon'");
		// LINE => ("line", "'line'");
		// UNKNOWN => ("unknown", "'unknown'");
		// CIRCLE => ("circle", "'circle'");
		// ACLITEM => ("aclitem", "'aclitem'");
		// BPCHAR => ("bpchar", "'bpchar'");
		// INTERVAL => ("interval", "'interval'");
		// BIT => ("bit", "'bit'");
		// VARBIT => ("varbit", "'varbit'");
		// REFCURSOR => ("refcursor", "'refcursor'");
		// REGPROCEDURE => ("regprocedure", "'regprocedure'");
		// REGOPER => ("regoper", "'regoper'");
		// REGOPERATOR => ("regoperator", "'regoperator'");
		// REGCLASS => ("regclass", "'regclass'");
		// REGTYPE => ("regtype", "'regtype'");
		// RECORD => ("record", "'record'");
		// CSTRING => ("cstring", "'cstring'");
		// ANY => ("any", "'any'");
		// ANYARRAY => ("anyarray", "'anyarray'");
		// TRIGGER => ("trigger", "'trigger'");
		// LANGUAGE_HANDLER => ("language_handler", "'language_handler'");
		// INTERNAL => ("internal", "'internal'");
		// ANYELEMENT => ("anyelement", "'anyelement'");
		// ANYNONARRAY => ("anynonarray", "'anynonarray'");
		// TXID_SNAPSHOT => ("txid_snapshot", "'txid_snapshot'");
		// FDW_HANDLER => ("fdw_handler", "'fdw_handler'");
		// PG_LSN => ("pg_lsn", "'pg_lsn'");
		// TSM_HANDLER => ("tsm_handler", "'tsm_handler'");
		// PG_NDISTINCT => ("pg_ndistinct", "'pg_ndistinct'");
		// PG_DEPENDENCIES => ("pg_dependencies", "'pg_dependencies'");
		// ANYENUM => ("anyenum", "'anyenum'");
		// TS_VECTOR => ("ts_vector", "'ts_vector'");
		// TSQUERY => ("tsquery", "'tsquery'");
		// GTS_VECTOR => ("gts_vector", "'gts_vector'");
		// REGCONFIG => ("regconfig", "'regconfig'");
		// REGDICTIONARY => ("regdictionary", "'regdictionary'");
		// ANY_RANGE => ("any_range", "'any_range'");
		// EVENT_TRIGGER => ("event_trigger", "'event_trigger'");
		// INT4_RANGE => ("int4_range", "'int4_range'");
		// NUM_RANGE => ("num_range", "'num_range'");
		// TS_RANGE => ("ts_range", "'ts_range'");
		// TSTZ_RANGE => ("tstz_range", "'tstz_range'");
		// DATE_RANGE => ("date_range", "'date_range'");
		// INT8_RANGE => ("int8_range", "'int8_range'");
		// JSONPATH => ("jsonpath", "'jsonpath'");
		// REGNAMESPACE => ("regnamespace", "'regnamespace'");
		// REGROLE => ("regrole", "'regrole'");
		// REGCOLLATION => ("regcollation", "'regcollation'");
		// INT4MULTI_RANGE => ("int4multi_range", "'int4multi_range'");
		// NUMMULTI_RANGE => ("nummulti_range", "'nummulti_range'");
		// TSMULTI_RANGE => ("tsmulti_range", "'tsmulti_range'");
		// TSTZMULTI_RANGE => ("tstzmulti_range", "'tstzmulti_range'");
		// DATEMULTI_RANGE => ("datemulti_range", "'datemulti_range'");
		// INT8MULTI_RANGE => ("int8multi_range", "'int8multi_range'");
		// ANYMULTI_RANGE => ("anymulti_range", "'anymulti_range'");
		// ANYCOMPATIBLEMULTI_RANGE => ("anycompatiblemulti_range", "'anycompatiblemulti_range'");
		// PG_BRIN_BLOOM_SUMMARY => ("pg_brin_bloom_summary", "'pg_brin_bloom_summary'");
		// PG_BRIN_MINMAX_MULTI_SUMMARY => ("pg_brin_minmax_multi_summary", "'pg_brin_minmax_multi_summary'");
		// PG_MCV_LIST => ("pg_mcv_list", "'pg_mcv_list'");
		// PG_SNAPSHOT => ("pg_snapshot", "'pg_snapshot'");
		// XID8 => ("xid8", "'xid8'");
		// ANYCOMPATIBLE => ("anycompatible", "'anycompatible'");
		// ANYCOMPATIBLEARRAY => ("anycompatiblearray", "'anycompatiblearray'");
		// ANYCOMPATIBLENONARRAY => ("anycompatiblenonarray", "'anycompatiblenonarray'");
		// ANYCOMPATIBLE_RANGE => ("anycompatible_range", "'anycompatible_range'");
	)
}
