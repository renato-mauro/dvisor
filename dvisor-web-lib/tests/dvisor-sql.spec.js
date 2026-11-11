import { test, expect } from '@playwright/test';

test('execSql compiles SQL and produces the same result as the equivalent JSON query', async ({ page }) => {
    await page.goto('/tests/fixtures/index.html');
    await page.waitForFunction(() => window.__ready === true);

    const origin = new URL(page.url()).origin;
    const from = `${origin}/tests/data/lineitem-sf1.parquet`;

    const jsonQuery = {
        from,
        filter: [
            { expr: 'l_shipdate', oper: 'GE', value: '1993-01-01' },
            { expr: 'l_shipdate', oper: 'LE', value: '1994-12-31' },
        ],
        project: [
            { expr: 'dateTruncMonth(l_shipdate)', alias: 'month' },
            { expr: 'sum(l_extendedprice*(1-l_discount))', alias: 'revenue' },
            { expr: 'sum(l_quantity)', alias: 'total_qty' },
            { expr: 'count()', alias: 'rows_scanned' }
        ],
        sort: [ { expr: 'month', direction: 'ASC' } ]
    };

    const sql = `
        SELECT
            dateTruncMonth(l_shipdate) AS month,
            sum(l_extendedprice*(1-l_discount)) AS revenue,
            sum(l_quantity) AS total_qty,
            count() AS rows_scanned
        FROM '${from}'
        WHERE l_shipdate >= '1993-01-01' AND l_shipdate <= '1994-12-31'
        ORDER BY month ASC
    `;

    const [jsonResult, sqlResult] = await Promise.all([
        page.evaluate((q) => window.runQuery(q), jsonQuery),
        page.evaluate((s) => window.runSql(s), sql),
    ]);

    expect(Array.isArray(sqlResult)).toBe(true);
    expect(sqlResult.length).toBeGreaterThan(0);
    expect(sqlResult.length).toBe(jsonResult.length);

    for (let i = 0; i < jsonResult.length; i++) {
        expect(String(sqlResult[i].month)).toBe(String(jsonResult[i].month));
        expect(sqlResult[i].revenue).toBeCloseTo(jsonResult[i].revenue, 5);
        expect(sqlResult[i].total_qty).toBeCloseTo(jsonResult[i].total_qty, 5);
        expect(sqlResult[i].rows_scanned).toBe(jsonResult[i].rows_scanned);
    }
});

test('execSql rejects unsupported SQL (OR in WHERE) with a clear error', async ({ page }) => {
    await page.goto('/tests/fixtures/index.html');
    await page.waitForFunction(() => window.__ready === true);

    const origin = new URL(page.url()).origin;
    const from = `${origin}/tests/data/lineitem-sf1.parquet`;

    const sql = `SELECT l_orderkey FROM '${from}' WHERE l_shipdate = '1993-01-01' OR l_shipdate = '1993-01-02'`;

    await expect(page.evaluate((s) => window.runSql(s), sql)).rejects.toBeTruthy();
});
