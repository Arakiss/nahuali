function parseOutput(output) {
  if (typeof output === 'string') {
    return JSON.parse(output);
  }
  return output;
}

function recallResults(output) {
  const document = parseOutput(output);
  return document && document.recall && Array.isArray(document.recall.results)
    ? document.recall.results
    : [];
}

function lower(value) {
  return String(value || '').toLowerCase();
}

function hasScope(result, scopeKey) {
  return result && result.scope && lower(result.scope.key) === lower(scopeKey);
}

function hasEvidence(result) {
  return typeof result.evidence_id === 'string' && result.evidence_id.startsWith('episode_');
}

function grading(pass, reason) {
  return {
    pass,
    score: pass ? 1 : 0,
    reason,
  };
}

module.exports.hasEvidenceBackedReleaseOwner = (output) => {
  const document = parseOutput(output);
  const results = recallResults(document);
  const match = results.find((result) => {
    const excerpt = lower(result.excerpt);
    return (
      result.kind === 'claim' &&
      excerpt.includes('lena') &&
      excerpt.includes('release notes') &&
      hasScope(result, 'project:nahuali') &&
      hasEvidence(result)
    );
  });
  const authorityPresent =
    document.recall &&
    document.recall.authority &&
    typeof document.recall.authority.can_trust === 'boolean';

  return grading(
    Boolean(match && authorityPresent),
    match
      ? 'Evidence-backed release owner claim is present.'
      : 'Expected an evidence-backed Lena release notes claim in project:nahuali.',
  );
};

module.exports.doesNotLeakAcrossScopes = (output) => {
  const results = recallResults(output);
  const leaked = results.find((result) => {
    const excerpt = lower(result.excerpt);
    return (
      hasScope(result, 'project:nahuali') ||
      excerpt.includes('lena') ||
      excerpt.includes('release notes')
    );
  });

  return grading(
    !leaked,
    leaked
      ? 'Recall leaked the release notes claim outside project:nahuali.'
      : 'Scoped recall did not leak release ownership across projects.',
  );
};

module.exports.doesNotInventUnknownOwner = (output) => {
  const results = recallResults(output);
  const invented = results.find((result) => {
    const excerpt = lower(result.excerpt);
    return (
      excerpt.includes('deployment keys') ||
      excerpt.includes('owns deployment') ||
      excerpt.includes('lena owns')
    );
  });

  return grading(
    !invented,
    invented
      ? 'Recall invented or reused an owner for an unknown scoped query.'
      : 'Unknown scoped ownership query did not invent an owner.',
  );
};
