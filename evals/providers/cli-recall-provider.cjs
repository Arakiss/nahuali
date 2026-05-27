const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

let callCounter = 0;

function repoRoot() {
  return path.resolve(__dirname, '..', '..');
}

function resolveBinary(config) {
  const configured =
    process.env.NAHUALI_EVAL_NAHUALI_BIN || config.binary || 'target/debug/nahuali';
  return path.isAbsolute(configured) ? configured : path.join(repoRoot(), configured);
}

function runNahuali(binary, args) {
  const result = spawnSync(binary, args, {
    cwd: repoRoot(),
    env: process.env,
    encoding: 'utf8',
    maxBuffer: 10 * 1024 * 1024,
  });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    throw new Error(
      [
        `nahuali exited with status ${result.status}`,
        `args: ${args.join(' ')}`,
        result.stdout ? `stdout:\n${result.stdout}` : '',
        result.stderr ? `stderr:\n${result.stderr}` : '',
      ]
        .filter(Boolean)
        .join('\n'),
    );
  }

  return result.stdout.trim();
}

function flagEnabled(value, defaultValue) {
  if (value === undefined || value === null || value === '') {
    return defaultValue;
  }
  if (typeof value === 'boolean') {
    return value;
  }
  return ['1', 'true', 'yes', 'on'].includes(String(value).toLowerCase());
}

function asList(value) {
  if (Array.isArray(value)) {
    return value.filter(Boolean).map(String);
  }
  if (value === undefined || value === null || value === '') {
    return [];
  }
  return [String(value)];
}

function seedRecallFixture(binary, database) {
  runNahuali(binary, [
    '--database',
    database,
    'remember',
    'Lena owns the release notes and keeps the changelog concise.',
    '--scope',
    'project:Nahuali',
    '--tag',
    'product',
    '--mention',
    'Lena',
    '--mention',
    'Release Notes',
  ]);
  runNahuali(binary, [
    '--database',
    database,
    'claim',
    'Lena',
    'owns',
    'release notes',
    '--scope',
    'project:Nahuali',
    '--confidence',
    '0.92',
    '--source-last',
  ]);
  runNahuali(binary, [
    '--database',
    database,
    'link',
    'Lena',
    'owns',
    'Release Notes',
    '--scope',
    'project:Nahuali',
    '--confidence',
    '0.9',
    '--source-last',
  ]);
  runNahuali(binary, [
    '--database',
    database,
    'preference',
    'Release Notes',
    'Keep release notes concise and evidence-backed.',
    '--scope',
    'project:Nahuali',
    '--source-last',
  ]);
  runNahuali(binary, [
    '--database',
    database,
    'remember',
    'Mira owns the billing checklist.',
    '--scope',
    'project:Billing',
    '--tag',
    'billing',
    '--mention',
    'Mira',
  ]);
  runNahuali(binary, [
    '--database',
    database,
    'claim',
    'Mira',
    'owns',
    'billing checklist',
    '--scope',
    'project:Billing',
    '--confidence',
    '0.88',
    '--source-last',
  ]);
  runNahuali(binary, [
    '--database',
    database,
    'link',
    'Mira',
    'owns',
    'Billing Checklist',
    '--scope',
    'project:Billing',
    '--confidence',
    '0.88',
    '--source-last',
  ]);
}

module.exports = class NahualiCliRecallProvider {
  constructor(options = {}) {
    this.config = options.config || {};
    this.binary = resolveBinary(this.config);
  }

  id() {
    return 'nahuali-cli-recall';
  }

  async callApi(prompt, context = {}) {
    if (!fs.existsSync(this.binary)) {
      throw new Error(`nahuali binary not found at ${this.binary}. Run cargo build -p nahuali-cli first.`);
    }

    const vars = context.vars || {};
    const database = `promptfoo_recall_${Date.now()}_${process.pid}_${++callCounter}`;
    const query = String(vars.query || prompt || '');
    const scope = String(vars.scope || 'project:Nahuali');
    const kinds = asList(vars.kinds || vars.kind || 'claim');
    const recallArgs = [
      '--database',
      database,
      'recall',
      query,
      '--scope',
      scope,
      '--json',
    ];

    for (const kind of kinds) {
      recallArgs.push('--kind', kind);
    }
    if (flagEnabled(vars.requireEvidence, true)) {
      recallArgs.push('--require-evidence');
    }
    if (flagEnabled(vars.authority, true)) {
      recallArgs.push('--authority');
    }
    if (flagEnabled(vars.semantic, false)) {
      recallArgs.push('--semantic');
    }

    seedRecallFixture(this.binary, database);

    const recall = JSON.parse(runNahuali(this.binary, recallArgs));
    return {
      output: JSON.stringify(
        {
          query,
          scope,
          recall,
        },
        null,
        2,
      ),
    };
  }
};
