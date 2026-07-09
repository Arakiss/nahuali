# Compliance Pack

This directory contains public, source-verified mapping documents for Nahuali's
current shipped behavior:

- [Threat Model](threat-model.md): trust boundaries, ledger integrity,
  Ed25519 tip attestation, attacker assumptions, and known limitations.
- [EU AI Act Article 12 Mapping](eu-ai-act-article-12.md): technical logging
  support mapped to Article 12 and related Articles 19 and 26.
- [OWASP ASI06 Mapping](owasp-asi06.md): memory and context poisoning controls
  mapped to the OWASP Top 10 for Agentic Applications.
- [Licensing FAQ](licensing-faq.md): practical summary of the
  source-available FSL-1.1-MIT license posture.
- [FINRA / SEC Books-and-Records Mapping](finserv-books-and-records.md): FINRA
  Rule 3110 supervision and SEC Rule 17a-4 electronic-records duties (including
  the 2022 audit-trail alternative to WORM) mapped to the ledger.
- [GDPR Position](gdpr-position.md): honest position on personal data in memory,
  the local-first deployment pattern, and the append-only ledger vs Article 17
  erasure problem.
- [Pilot Data Policy](pilot-data-policy.md): what a pilot deployment may and must
  not store, isolation, backup/restore/reconcile safety, and end-of-pilot data
  disposition.
- [Security Review Answer Pack](security-questionnaire.md): pre-filled answers to
  an AI-vendor security review — auditability, policy enforcement, data
  residency, key custody, authentication posture, and supply chain.

All public compliance and pilot collateral lives flat under `compliance/`; there
is no nested `docs/` layout for these buyer-facing documents.

These are alignment and mapping documents, not a certification, legal opinion,
or third-party audit. Nahuali is source-available (FSL-1.1-MIT). Verify every
implementation claim against the cited code and check legal or regulatory
interpretations with counsel before relying on them in a regulated deployment.
