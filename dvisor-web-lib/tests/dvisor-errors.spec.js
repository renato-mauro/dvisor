import { test, expect } from '@playwright/test';

// The error contract of a query iterator: every failure rejects the next
// next() exactly once with a DvisorError whose `kind` names the phase it
// happened in ('init' | 'query' | 'execution'), the query is cancelled in
// every worker, and the Dvisor instance stays usable afterwards. The
// failures themselves are injected by tests/server.mjs (see its ALIASES).

const DATA = 'lineitem-sf1';
const sumQuantity = (from) => `SELECT sum(l_quantity) AS q FROM '${from}'`;

async function open(page, lib) {
    await page.goto('/tests/fixtures/errors.html' + (lib ? `?lib=${lib}` : ''));
    await page.waitForFunction(() => window.__ready === true);
    return new URL(page.url()).origin;
}

function expectReadableMessage(result) {
    expect(typeof result.message).toBe('string');
    expect(result.message.length).toBeGreaterThan(0);
    expect(result.message).not.toMatch(/JsValue\(/);
}

test('invalid SQL rejects with kind "query" even when nobody is awaiting next() yet', async ({ page }) => {
    const origin = await open(page);
    const from = `${origin}/tests/data/${DATA}.parquet`;

    const result = await page.evaluate(async (from) => {
        const dv = new Dvisor(4);
        // OR in WHERE is unsupported (see dvisor-sql.spec.js).
        const it = dv.execSql(`SELECT l_orderkey FROM '${from}' WHERE l_shipdate = '1993-01-01' OR l_shipdate = '1993-01-02'`);
        const first = await settle(it, { delayMs: 1500 });
        const after = await it.next();
        return { first, afterDone: after.done };
    }, from);

    expect(result.first).toMatchObject({ outcome: 'rejected', isDvisorError: true, kind: 'query' });
    expectReadableMessage(result.first);
    expect(result.afterDone).toBe(true);
});

test('a missing parquet file rejects with kind "query"', async ({ page }) => {
    const origin = await open(page);
    const from = `${origin}/tests/no-data/${DATA}.parquet`;

    const result = await page.evaluate((sql) => settle(new Dvisor(2).execSql(sql)), sumQuantity(from));

    expect(result).toMatchObject({ outcome: 'rejected', isDvisorError: true, kind: 'query' });
    expectReadableMessage(result);
    expect(result.message).toMatch(/404/);
});

test('a failure during execution rejects with kind "execution" and leaves the instance usable', async ({ page }) => {
    const origin = await open(page);
    const broken = sumQuantity(`${origin}/tests/broken-data/${DATA}.parquet`);
    const healthy = sumQuantity(`${origin}/tests/data/${DATA}.parquet`);

    const result = await page.evaluate(async ({ broken, healthy }) => {
        const dv = new Dvisor(2);
        return {
            broken: await settle(dv.execSql(broken)),
            // The failed query must not linger in the workers: a later query
            // on the same instance has to run to completion.
            healthy: await settle(dv.execSql(healthy)),
        };
    }, { broken, healthy });

    expect(result.broken).toMatchObject({ outcome: 'rejected', isDvisorError: true, kind: 'execution' });
    expectReadableMessage(result.broken);
    expect(result.broken.message).toMatch(/503/);
    expect(result.healthy).toMatchObject({ outcome: 'completed', rows: 1 });
});

test('a failure during execution is not lost when nobody is awaiting next() yet', async ({ page }) => {
    const origin = await open(page);
    const broken = sumQuantity(`${origin}/tests/broken-data/${DATA}.parquet`);

    const result = await page.evaluate((sql) => settle(new Dvisor(2).execSql(sql), { delayMs: 1500 }), broken);

    expect(result).toMatchObject({ outcome: 'rejected', isDvisorError: true, kind: 'execution' });
});

test('a failed initialization rejects ready and every request with kind "init" instead of hanging', async ({ page }) => {
    const origin = await open(page, '/tests/no-wasm/index.js');
    const from = `${origin}/tests/data/${DATA}.parquet`;

    const result = await page.evaluate(async (from) => {
        const dv = new Dvisor(2);
        const asOutcome = (p) => withTimeout(
            Promise.resolve(p).then(() => ({ outcome: 'resolved' }), describeError), 10_000);
        return {
            ready: await asOutcome(dv.ready),
            query: await settle(dv.execSql(`SELECT sum(l_quantity) AS q FROM '${from}'`), { timeoutMs: 10_000 }),
            metadata: await asOutcome(dv.createMetadata(from)),
        };
    }, from);

    for (const outcome of [result.ready, result.query, result.metadata]) {
        expect(outcome).toMatchObject({ outcome: 'rejected', isDvisorError: true, kind: 'init' });
        expectReadableMessage(outcome);
    }
});

test('a page whose CSP forbids eval gets an error, not an empty result', async ({ page }) => {
    await page.goto('/tests/fixtures/csp-no-eval.html');
    await page.waitForFunction(() => window.result);

    const result = await page.evaluate(() => window.result);

    expect(result).toMatchObject({ outcome: 'rejected', isDvisorError: true, kind: 'execution' });
    expect(result.message).toMatch(/unsafe-eval/);
});

test('cancelling one of two concurrent queries does not stall the other', async ({ page }) => {
    const origin = await open(page);
    const sql = sumQuantity(`${origin}/tests/data/${DATA}.parquet`);

    // With one worker both queries share its round-robin. The bug this
    // guards against only bit when the cancel arrived while the worker's
    // cursor pointed at the cancelled query, so the cancel point is swept
    // across several delays rather than relying on a single timing.
    const rounds = await page.evaluate(async (sql) => {
        const dv = new Dvisor(1);
        const rounds = [];
        for (const delayMs of [0, 2, 4, 6, 8, 10, 12, 14]) {
            const [kept, cancelled] = await Promise.all([
                settle(dv.execSql(sql), { timeoutMs: 15_000 }),
                settle(dv.execSql(sql), { delayMs, breakAfter: 1 }),
            ]);
            rounds.push({ delayMs, kept: kept.outcome, cancelled: cancelled.outcome });
            if (kept.outcome !== 'completed') break;
        }
        return rounds;
    }, sql);

    for (const round of rounds) {
        expect(round).toMatchObject({ kept: 'completed', cancelled: 'completed' });
    }
    expect(rounds.length).toBe(8);
});
