//! SurrealDB schema artifacts used by the local memory runtime.

/// Authoritative SurrealDB schema for the v1 append-only record ledger.
pub const MEMORY_RECORD_SCHEMA: &str = include_str!("../schema/memory_record.surql");

/// Rebuildable SurrealDB graph projection schema for the beta memory substrate.
pub const GRAPH_PROJECTION_SCHEMA: &str = include_str!("../schema/graph_projection_v1.surql");

#[cfg(test)]
mod tests {
    use super::{GRAPH_PROJECTION_SCHEMA, MEMORY_RECORD_SCHEMA};

    #[test]
    fn memory_record_schema_defines_only_the_v1_ledger_contract() {
        let schema = MEMORY_RECORD_SCHEMA.trim();

        assert!(schema.contains("DEFINE TABLE IF NOT EXISTS memory_record SCHEMALESS;"));
        assert!(
            schema.contains(
                "DEFINE INDEX IF NOT EXISTS memory_record_sequence_idx ON TABLE memory_record COLUMNS sequence UNIQUE;"
            )
        );
        assert!(!schema.contains("DEFINE TABLE IF NOT EXISTS entity"));
        assert!(!schema.contains("DEFINE TABLE IF NOT EXISTS episode"));
        assert!(!schema.contains("DEFINE TABLE IF NOT EXISTS relates_to"));
    }

    #[test]
    fn graph_projection_schema_defines_the_beta_cognitive_graph_contract() {
        let schema = GRAPH_PROJECTION_SCHEMA.trim();

        for table in [
            "projection_checkpoint",
            "projection_error",
            "memory_scope",
            "source_record",
            "episode",
            "entity",
            "claim",
            "procedure",
            "intention",
            "health_signal",
            "review_item",
            "review_decision",
            "inferred_claim",
            "contradiction",
            "anomaly_alert",
        ] {
            assert!(
                schema.contains(&format!("DEFINE TABLE IF NOT EXISTS {table}")),
                "missing graph projection table {table}"
            );
        }

        assert!(
            schema.contains(
                "DEFINE TABLE IF NOT EXISTS mentions TYPE RELATION IN episode OUT entity;"
            )
        );
        assert!(schema.contains("DEFINE TABLE IF NOT EXISTS supports TYPE RELATION IN claim|procedure|intention|review_decision OUT episode|source_record;"));
        assert!(
            schema.contains(
                "DEFINE TABLE IF NOT EXISTS relates_to TYPE RELATION IN entity OUT entity;"
            )
        );
        assert!(schema.contains("DEFINE TABLE IF NOT EXISTS intention_depends_on TYPE RELATION IN intention OUT intention;"));
    }
}
