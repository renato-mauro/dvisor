import { test, expect } from '@playwright/test';

test('dvisor query end-to-end against a real parquet dataset', async ({ page }) => {
    await page.goto('/tests/fixtures/index.html');
    await page.waitForFunction(() => window.__ready === true);

    const origin = new URL(page.url()).origin;

    const query = {
        from: `${origin}/tests/data/lineitem-sf1.parquet`,
        filter: [
            { expr: 'l_shipdate', oper: 'GE', value: '1993-01-01' },
            { expr: 'l_shipdate', oper: 'LE', value: '1994-12-31' },
        ],
        project: [
            { expr: 'dateTruncMonth(l_shipdate)', alias: 'month' },
            { expr: 'sum(l_extendedprice*(1-l_discount))', alias: 'revenue' },
            { expr: 'sum(l_quantity)', alias: 'total_qty' },
            { expr: 'count()', alias: 'rows_scanned' }
        ]
    };

    const result = await page.evaluate((q) => window.runQuery(q), query);

    expect(Array.isArray(result)).toBe(true);
    expect(result.length).toBeGreaterThan(0);

    let totalRows = 0;
    for (const row of result) {
        expect(row).toHaveProperty('month');
        expect(typeof row.revenue).toBe('number');
        expect(typeof row.total_qty).toBe('number');
        expect(typeof row.rows_scanned).toBe('number');
        expect(row.revenue).toBeGreaterThan(0);
        expect(row.rows_scanned).toBeGreaterThan(0);
        totalRows += row.rows_scanned;
    }
    expect(totalRows).toBeGreaterThan(0);
});
