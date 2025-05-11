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

pub fn base_pg_type_to_ts_info(typ: &postgres::types::Type) -> &str {
	match_pg_type!(*typ,
		TEXT => "'string'";
		VARCHAR => "'string'";
		JSON => "'json'";
		JSONB => "'json'";
		BOOL => "'bool'";
		INT8 => "'number'";
		INT2 => "'number'";
		INT4 => "'number'";
		NUMERIC => "'number'";
		MONEY => "'number'";

		// BYTEA => "'bytea'";
		// CHAR => "'char'";
		// FLOAT4 => "'float4'";
		// FLOAT8 => "'float8'";

		// DATE => "'date'";
		// TIME => "'time'";
		// TIMESTAMP => "'timestamp'";
		// TIMESTAMPTZ => "'timestamptz'";

		// CIDR => "'cidr'";
		// INET => "'inet'";
		// TIMETZ => "'timetz'";
		// VOID => "'void'";

		// MACADDR8 => "'macaddr8'";
		// MACADDR => "'macaddr'";
		// XML => "'xml'";
		// UUID => "'uuid'";

		// INT2_VECTOR => "'int2_vector'";
		// REGPROC => "'regproc'";
		// NAME => "'name'";
		// OID => "'oid'";
		// TID => "'tid'";
		// XID => "'xid'";
		// CID => "'cid'";
		// OID_VECTOR => "'oid_vector'";
		// PG_DDL_COMMAND => "'pg_ddl_command'";
		// PG_NODE_TREE => "'pg_node_tree'";
		// TABLE_AM_HANDLER => "'table_am_handler'";
		// INDEX_AM_HANDLER => "'index_am_handler'";
		// POINT => "'point'";
		// LSEG => "'lseg'";
		// PATH => "'path'";
		// BOX => "'box'";
		// POLYGON => "'polygon'";
		// LINE => "'line'";
		// UNKNOWN => "'unknown'";
		// CIRCLE => "'circle'";
		// ACLITEM => "'aclitem'";
		// BPCHAR => "'bpchar'";
		// INTERVAL => "'interval'";
		// BIT => "'bit'";
		// VARBIT => "'varbit'";
		// REFCURSOR => "'refcursor'";
		// REGPROCEDURE => "'regprocedure'";
		// REGOPER => "'regoper'";
		// REGOPERATOR => "'regoperator'";
		// REGCLASS => "'regclass'";
		// REGTYPE => "'regtype'";
		// RECORD => "'record'";
		// CSTRING => "'cstring'";
		// ANY => "'any'";
		// ANYARRAY => "'anyarray'";
		// TRIGGER => "'trigger'";
		// LANGUAGE_HANDLER => "'language_handler'";
		// INTERNAL => "'internal'";
		// ANYELEMENT => "'anyelement'";
		// ANYNONARRAY => "'anynonarray'";
		// TXID_SNAPSHOT => "'txid_snapshot'";
		// FDW_HANDLER => "'fdw_handler'";
		// PG_LSN => "'pg_lsn'";
		// TSM_HANDLER => "'tsm_handler'";
		// PG_NDISTINCT => "'pg_ndistinct'";
		// PG_DEPENDENCIES => "'pg_dependencies'";
		// ANYENUM => "'anyenum'";
		// TS_VECTOR => "'ts_vector'";
		// TSQUERY => "'tsquery'";
		// GTS_VECTOR => "'gts_vector'";
		// REGCONFIG => "'regconfig'";
		// REGDICTIONARY => "'regdictionary'";
		// ANY_RANGE => "'any_range'";
		// EVENT_TRIGGER => "'event_trigger'";
		// INT4_RANGE => "'int4_range'";
		// NUM_RANGE => "'num_range'";
		// TS_RANGE => "'ts_range'";
		// TSTZ_RANGE => "'tstz_range'";
		// DATE_RANGE => "'date_range'";
		// INT8_RANGE => "'int8_range'";
		// JSONPATH => "'jsonpath'";
		// REGNAMESPACE => "'regnamespace'";
		// REGROLE => "'regrole'";
		// REGCOLLATION => "'regcollation'";
		// INT4MULTI_RANGE => "'int4multi_range'";
		// NUMMULTI_RANGE => "'nummulti_range'";
		// TSMULTI_RANGE => "'tsmulti_range'";
		// TSTZMULTI_RANGE => "'tstzmulti_range'";
		// DATEMULTI_RANGE => "'datemulti_range'";
		// INT8MULTI_RANGE => "'int8multi_range'";
		// ANYMULTI_RANGE => "'anymulti_range'";
		// ANYCOMPATIBLEMULTI_RANGE => "'anycompatiblemulti_range'";
		// PG_BRIN_BLOOM_SUMMARY => "'pg_brin_bloom_summary'";
		// PG_BRIN_MINMAX_MULTI_SUMMARY => "'pg_brin_minmax_multi_summary'";
		// PG_MCV_LIST => "'pg_mcv_list'";
		// PG_SNAPSHOT => "'pg_snapshot'";
		// XID8 => "'xid8'";
		// ANYCOMPATIBLE => "'anycompatible'";
		// ANYCOMPATIBLEARRAY => "'anycompatiblearray'";
		// ANYCOMPATIBLENONARRAY => "'anycompatiblenonarray'";
		// ANYCOMPATIBLE_RANGE => "'anycompatible_range'";
	)
}
