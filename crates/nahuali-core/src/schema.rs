//! SurrealDB schema artifacts used by the local memory runtime.

/// Authoritative SurrealDB schema for the v1 append-only record ledger.
pub const MEMORY_RECORD_SCHEMA: &str = include_str!("../schema/memory_record.surql");

/// Historical v1 graph projection schema retained for compatibility evidence.
#[cfg(test)]
pub const GRAPH_PROJECTION_SCHEMA_V1: &str = include_str!("../schema/graph_projection_v1.surql");

/// Rebuildable SurrealDB graph projection schema for the beta memory substrate.
pub const GRAPH_PROJECTION_SCHEMA: &str = include_str!("../schema/graph_projection_v2.surql");

#[cfg(test)]
mod tests {
    use super::{GRAPH_PROJECTION_SCHEMA, GRAPH_PROJECTION_SCHEMA_V1, MEMORY_RECORD_SCHEMA};

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

        assert_ne!(schema, GRAPH_PROJECTION_SCHEMA_V1.trim());
        assert!(schema.contains("Graph projection v2"));
        assert!(
            schema.contains(
                "DEFINE SEQUENCE IF NOT EXISTS projection_rebuild_fencing BATCH 1 START 1;"
            )
        );
        assert!(schema.contains(
            "DEFINE SEQUENCE IF NOT EXISTS projection_rebuild_mutation_guard BATCH 1 START 1;"
        ));

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
