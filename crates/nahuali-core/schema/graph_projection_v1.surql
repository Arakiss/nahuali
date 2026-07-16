DEFINE TABLE IF NOT EXISTS projection_checkpoint SCHEMALESS;
DEFINE INDEX IF NOT EXISTS projection_checkpoint_version_idx ON TABLE projection_checkpoint COLUMNS projection_version UNIQUE;

DEFINE TABLE IF NOT EXISTS projection_rebuild_lock SCHEMALESS;

DEFINE TABLE IF NOT EXISTS projection_error SCHEMALESS;
DEFINE INDEX IF NOT EXISTS projection_error_sequence_idx ON TABLE projection_error COLUMNS source_sequence;

DEFINE TABLE IF NOT EXISTS memory_scope SCHEMALESS;
DEFINE INDEX IF NOT EXISTS memory_scope_key_idx ON TABLE memory_scope COLUMNS scope_key UNIQUE;

DEFINE TABLE IF NOT EXISTS source_record SCHEMALESS;
DEFINE INDEX IF NOT EXISTS source_record_memory_id_idx ON TABLE source_record COLUMNS memory_id UNIQUE;
DEFINE INDEX IF NOT EXISTS source_record_scope_idx ON TABLE source_record COLUMNS scope_key;

DEFINE TABLE IF NOT EXISTS episode SCHEMALESS;
DEFINE INDEX IF NOT EXISTS episode_memory_id_idx ON TABLE episode COLUMNS memory_id UNIQUE;
DEFINE INDEX IF NOT EXISTS episode_event_idx ON TABLE episode COLUMNS event_id UNIQUE;
DEFINE INDEX IF NOT EXISTS episode_scope_idx ON TABLE episode COLUMNS scope_key;

DEFINE TABLE IF NOT EXISTS entity SCHEMALESS;
DEFINE INDEX IF NOT EXISTS entity_memory_id_idx ON TABLE entity COLUMNS memory_id UNIQUE;
DEFINE INDEX IF NOT EXISTS entity_scope_idx ON TABLE entity COLUMNS scope_key;

DEFINE TABLE IF NOT EXISTS claim SCHEMALESS;
DEFINE INDEX IF NOT EXISTS claim_memory_id_idx ON TABLE claim COLUMNS memory_id UNIQUE;
DEFINE INDEX IF NOT EXISTS claim_event_idx ON TABLE claim COLUMNS event_id UNIQUE;
DEFINE INDEX IF NOT EXISTS claim_scope_idx ON TABLE claim COLUMNS scope_key;

DEFINE TABLE IF NOT EXISTS procedure SCHEMALESS;
DEFINE INDEX IF NOT EXISTS procedure_memory_id_idx ON TABLE procedure COLUMNS memory_id UNIQUE;
DEFINE INDEX IF NOT EXISTS procedure_event_idx ON TABLE procedure COLUMNS event_id UNIQUE;
DEFINE INDEX IF NOT EXISTS procedure_scope_idx ON TABLE procedure COLUMNS scope_key;

DEFINE TABLE IF NOT EXISTS intention SCHEMALESS;
DEFINE INDEX IF NOT EXISTS intention_memory_id_idx ON TABLE intention COLUMNS memory_id UNIQUE;
DEFINE INDEX IF NOT EXISTS intention_event_idx ON TABLE intention COLUMNS event_id UNIQUE;
DEFINE INDEX IF NOT EXISTS intention_scope_idx ON TABLE intention COLUMNS scope_key;

DEFINE TABLE IF NOT EXISTS health_signal SCHEMALESS;
DEFINE INDEX IF NOT EXISTS health_signal_memory_id_idx ON TABLE health_signal COLUMNS memory_id UNIQUE;
DEFINE INDEX IF NOT EXISTS health_signal_scope_idx ON TABLE health_signal COLUMNS scope_key;

DEFINE TABLE IF NOT EXISTS review_item SCHEMALESS;
DEFINE INDEX IF NOT EXISTS review_item_memory_id_idx ON TABLE review_item COLUMNS memory_id UNIQUE;
DEFINE INDEX IF NOT EXISTS review_item_scope_idx ON TABLE review_item COLUMNS scope_key;

DEFINE TABLE IF NOT EXISTS review_decision SCHEMALESS;
DEFINE INDEX IF NOT EXISTS review_decision_memory_id_idx ON TABLE review_decision COLUMNS memory_id UNIQUE;
DEFINE INDEX IF NOT EXISTS review_decision_event_idx ON TABLE review_decision COLUMNS event_id UNIQUE;
DEFINE INDEX IF NOT EXISTS review_decision_scope_idx ON TABLE review_decision COLUMNS scope_key;

DEFINE TABLE IF NOT EXISTS inferred_claim SCHEMALESS;
DEFINE INDEX IF NOT EXISTS inferred_claim_memory_id_idx ON TABLE inferred_claim COLUMNS memory_id UNIQUE;

DEFINE TABLE IF NOT EXISTS contradiction SCHEMALESS;
DEFINE INDEX IF NOT EXISTS contradiction_memory_id_idx ON TABLE contradiction COLUMNS memory_id UNIQUE;
DEFINE INDEX IF NOT EXISTS contradiction_scope_idx ON TABLE contradiction COLUMNS scope_key;

DEFINE TABLE IF NOT EXISTS anomaly_alert SCHEMALESS;
DEFINE INDEX IF NOT EXISTS anomaly_alert_memory_id_idx ON TABLE anomaly_alert COLUMNS memory_id UNIQUE;
DEFINE INDEX IF NOT EXISTS anomaly_alert_scope_idx ON TABLE anomaly_alert COLUMNS scope_key;

DEFINE TABLE IF NOT EXISTS mentions TYPE RELATION IN episode OUT entity;
DEFINE INDEX IF NOT EXISTS mentions_memory_id_idx ON TABLE mentions COLUMNS memory_id UNIQUE;
DEFINE INDEX IF NOT EXISTS mentions_episode_idx ON TABLE mentions COLUMNS episode_id;
DEFINE INDEX IF NOT EXISTS mentions_entity_idx ON TABLE mentions COLUMNS entity_id;

DEFINE TABLE IF NOT EXISTS supports TYPE RELATION IN claim|procedure|intention|review_decision OUT episode|source_record;
DEFINE INDEX IF NOT EXISTS supports_memory_id_idx ON TABLE supports COLUMNS memory_id UNIQUE;
DEFINE INDEX IF NOT EXISTS supports_subject_idx ON TABLE supports COLUMNS subject_id;
DEFINE INDEX IF NOT EXISTS supports_evidence_idx ON TABLE supports COLUMNS evidence_id;

DEFINE TABLE IF NOT EXISTS relates_to TYPE RELATION IN entity OUT entity;
DEFINE INDEX IF NOT EXISTS relates_to_memory_id_idx ON TABLE relates_to COLUMNS memory_id UNIQUE;
DEFINE INDEX IF NOT EXISTS relates_to_relation_idx ON TABLE relates_to COLUMNS relation;
DEFINE INDEX IF NOT EXISTS relates_to_scope_idx ON TABLE relates_to COLUMNS scope_key;

DEFINE TABLE IF NOT EXISTS intention_depends_on TYPE RELATION IN intention OUT intention;
DEFINE INDEX IF NOT EXISTS intention_depends_on_memory_id_idx ON TABLE intention_depends_on COLUMNS memory_id UNIQUE;
