// Computes the expected result of every case in cases.mjs with DuckDB, an
// engine independent of dvisor, and writes them to expected.json, so that
// the tests themselves don't need DuckDB. Run from dvisor-web-lib after
// changing a case or a data file:
//
//     node tests/grouping/make-expected.mjs
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { cases, files } from './cases.mjs';

const expected = {};
for (const c of cases) {
    const sql = c.duckdb.replaceAll('{FROM}', `'${files[c.file ?? 'grouped']}'`);
    expected[c.name] = JSON.parse(execFileSync('duckdb', ['-json', '-c', sql], { encoding: 'utf8' }));
}
writeFileSync(new URL('./expected.json', import.meta.url), JSON.stringify(expected, null, 1) + '\n');
console.log(`wrote expected results for ${cases.length} cases`);
