DEFINE TABLE IF NOT EXISTS memory_record SCHEMALESS;
DEFINE INDEX IF NOT EXISTS memory_record_sequence_idx ON TABLE memory_record COLUMNS sequence UNIQUE;
