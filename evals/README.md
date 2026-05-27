# Evals

This directory contains app-backed evals for user-visible memory behavior.

The first suite exercises the real `nahuali` CLI path for scoped,
evidence-backed recall. It seeds synthetic memory into an isolated database,
runs `nahuali recall --json`, and checks the JSON contract an operator or agent
host would consume.

Run:

```bash
bash scripts/verify-recall-evals.sh
```

The script builds the CLI, starts the local service stack, runs Promptfoo with a
custom JavaScript provider, and writes generated results only to a temporary
directory unless overridden by the operator environment.

Keep eval fixtures synthetic. Do not commit Promptfoo run results, private
memory exports, model traces, or customer data.
