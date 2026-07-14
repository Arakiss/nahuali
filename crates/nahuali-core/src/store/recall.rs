impl MemoryEngine {
    /// Recall memory items that match a lexical query.
    ///
    /// `limit` is coerced to at least `1`. Empty queries are rejected after
    /// trimming.
    pub fn recall(&self, query: &str, limit: usize) -> Result<Vec<RecallResult>> {
        self.recall_with_options(
            query,
            RecallOptions {
                limit,
                ..RecallOptions::default()
            },
        )
    }

    /// Recall memory items that match a lexical query inside one exact scope.
    pub fn recall_scoped(
        &self,
        query: &str,
        limit: usize,
        scope: &MemoryScope,
    ) -> Result<Vec<RecallResult>> {
        self.recall_with_options(
            query,
            RecallOptions {
                limit,
                scope: Some(scope.clone()),
                ..RecallOptions::default()
            },
        )
    }

    /// Recall memory items with explicit filtering options.
    pub fn recall_with_options(
        &self,
        query: &str,
        options: RecallOptions,
    ) -> Result<Vec<RecallResult>> {
        if query.trim().is_empty() {
            return Err(NahualiError::EmptyQuery);
        }

        Ok(recall::recall_with_options(&self.data, query, options))
    }

    /// Recall memory items and include authority context for the projection.
    pub fn recall_with_authority(&self, query: &str, limit: usize) -> Result<AuthorityRecall> {
        self.recall_with_authority_options(
            query,
            RecallOptions {
                limit,
                ..RecallOptions::default()
            },
        )
    }

    /// Recall scoped memory and include authority context for the projection.
    pub fn recall_scoped_with_authority(
        &self,
        query: &str,
        limit: usize,
        scope: &MemoryScope,
    ) -> Result<AuthorityRecall> {
        self.recall_with_authority_options(
            query,
            RecallOptions {
                limit,
                scope: Some(scope.clone()),
                ..RecallOptions::default()
            },
        )
    }

    /// Recall memory with explicit filtering options and authority context.
    pub fn recall_with_authority_options(
        &self,
        query: &str,
        options: RecallOptions,
    ) -> Result<AuthorityRecall> {
        recall::recall_projection_with_authority(&self.data, query, options)
    }
}
