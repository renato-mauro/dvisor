import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { cases } from './grouping/cases.mjs';

// Grouped queries merge the partial result of each row group into the
// result so far, by interleaving two lists sorted by the group columns. Each
// case in grouping/cases.mjs is checked against DuckDB's answer
// (grouping/expected.json), and every snapshot against the merge invariants.
// A case runs several times on an instance that has first run a query with a
// calculated column whose row groups all went to one worker: that is what
// used to leave the workers planning the same query differently.

const expected = JSON.parse(readFileSync(new URL('./grouping/expected.json', import.meta.url), 'utf8'));
const RUNS = 3;
const MAX_REPORTED = 8;

function fileUrl(origin, c) {
    return `${origin}/tests/data/${c.file === 'lineitem' ? 'lineitem-sf1' : 'grouped'}.parquet`;
}

function queryFor(c, from) {
    return c.sql
        ? c.sql.replaceAll('{FROM}', `'${from}'`)
        : JSON.parse(JSON.stringify(c.dsl).replaceAll('{FROM}', from));
}

// Sums and averages are computed in 64-bit floating point, and merged in an
// order that depends on which row group arrives first, so they match up to
// rounding.
function same(actual, wanted) {
    if (typeof actual === 'number' && typeof wanted === 'number') {
        return Math.abs(actual - wanted) <= 1e-9 * Math.max(1, Math.abs(wanted));
    }
    return actual === wanted;
}

function differences(actual, wanted, keys) {
    const keyOf = (row) => JSON.stringify(keys.map((k) => row[k]));
    const byKey = new Map(actual.map((row) => [keyOf(row), row]));
    const found = [];
    if (actual.length !== wanted.length) {
        found.push(`${actual.length} rows instead of ${wanted.length}`);
    }
    for (const w of wanted) {
        const a = byKey.get(keyOf(w));
        if (!a) {
            found.push(`group ${keyOf(w)} is missing`);
            continue;
        }
        for (const [column, value] of Object.entries(w)) {
            if (!same(a[column], value)) {
                found.push(`group ${keyOf(w)}: ${column} is ${JSON.stringify(a[column])}, expected ${JSON.stringify(value)}`);
            }
        }
    }
    return found;
}

function orderProblems(rows, order) {
    for (let i = 1; i < rows.length; i++) {
        for (const [column, direction] of order) {
            const a = rows[i - 1][column];
            const b = rows[i][column];
            if (a === b) continue;
            if ((a < b) !== (direction === 'asc')) {
                return [`rows ${i - 1} and ${i} are not ordered by ${column} ${direction}`];
            }
            break;
        }
    }
    return [];
}

async function openPage(page) {
    await page.goto('/tests/fixtures/grouping.html');
    await page.waitForFunction(() => window.__ready === true);
    return new URL(page.url()).origin;
}

test.describe('grouped queries merge partial results correctly', () => {
    for (const c of cases) {
        (c.fixme ? test.fixme : test)(c.name, async ({ page }) => {
            test.setTimeout(120_000);
            const origin = await openPage(page);
            const grouped = fileUrl(origin, {});
            for (const warmUp of [
                // The 2021 row groups (3, 7, 11, ...) all belong to worker 3.
                `SELECT year(dt) AS y, count() AS n FROM '${grouped}' WHERE dt >= '2021-01-01' GROUP BY 1`,
                `SELECT region, avg(weight) AS w, count() AS n FROM '${grouped}'`,
            ]) {
                await page.evaluate((q) => window.runChecked(q, []), warmUp);
            }

            const query = queryFor(c, fileUrl(origin, c));
            const failures = [];
            for (let run = 1; run <= RUNS; run++) {
                const result = await page.evaluate(([q, keys]) => window.runChecked(q, keys), [query, c.keys]);
                const found = result.error
                    ? [result.error]
                    : [
                        ...result.problems,
                        ...differences(result.rows, expected[c.name], c.keys),
                        ...orderProblems(result.rows, c.order ?? []),
                    ];
                failures.push(...found.slice(0, MAX_REPORTED).map((f) => `run ${run}: ${f}`));
            }
            expect(failures).toEqual([]);
        });
    }
});

test.describe('the JSON DSL rejects what it does not support', () => {
    const rejected = [
        {
            name: 'the removed rename field',
            query: (from) => ({
                from,
                project: [{ expr: 'region' }, { aggregate: 'count', alias: 'n' }],
                rename: ['r', 'count'],
            }),
            mentions: 'rename',
        },
        {
            name: 'an unknown field, such as a misspelled filter',
            query: (from) => ({
                from,
                filters: [{ expr: 'region', oper: 'EQ', value: 11 }],
                project: [{ aggregate: 'count', alias: 'n' }],
            }),
            mentions: 'filters',
        },
        {
            name: 'a sort on a column the query does not produce',
            query: (from) => ({
                from,
                project: [{ expr: 'region' }, { aggregate: 'count', alias: 'n' }],
                sort: [{ expr: 'total', direction: 'DESC' }],
            }),
            mentions: 'total',
        },
    ];
    for (const r of rejected) {
        test(r.name, async ({ page }) => {
            const origin = await openPage(page);
            const result = await page.evaluate((q) => window.runChecked(q, []), r.query(fileUrl(origin, {})));
            expect(result.error).toMatch(/^query: /);
            expect(result.error).toContain(r.mentions);
        });
    }
});
