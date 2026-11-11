import { test, expect } from '@playwright/test';

test('date/numeric api.js functions work through execSql', async ({ page }) => {
    await page.goto('/tests/fixtures/index.html');
    await page.waitForFunction(() => window.__ready === true);

    const origin = new URL(page.url()).origin;
    const from = `${origin}/tests/data/lineitem-sf1.parquet`;

    const sql = `
        SELECT
            upper(l_returnflag) AS flag,
            month(l_shipdate) AS ship_month,
            quarter(l_shipdate) AS ship_quarter,
            dayOfWeek(l_shipdate) AS ship_dow,
            first(dateTruncQuarter(l_shipdate)) AS quarter_start,
            avg(l_extendedprice) AS avg_price,
            max(ceil(l_discount*100)) AS max_discount_pct,
            min(floor(l_quantity)) AS min_qty,
            avg(abs(l_discount)) AS avg_abs_discount,
            max(sqrt(l_quantity)) AS max_qty_sqrt,
            max(mod(l_orderkey, 7)) AS max_orderkey_mod7,
            avg(dateDiffDays(l_shipdate, l_receiptdate)) AS avg_ship_to_receipt_days,
            first(dateAddDays(l_shipdate, 7)) AS ship_plus_week,
            count() AS rows_scanned
        FROM '${from}'
        WHERE l_shipdate >= '1993-01-01' AND l_shipdate <= '1993-03-31'
        GROUP BY upper(l_returnflag), month(l_shipdate), quarter(l_shipdate), dayOfWeek(l_shipdate)
        ORDER BY ship_month ASC
    `;

    const result = await page.evaluate((s) => window.runSql(s), sql);

    expect(Array.isArray(result)).toBe(true);
    expect(result.length).toBeGreaterThan(0);

    for (const row of result) {
        expect(row.flag).toMatch(/^[A-Z]$/);
        expect([1, 2, 3]).toContain(row.ship_month);
        expect(row.ship_quarter).toBe(1);
        expect(row.ship_dow).toBeGreaterThanOrEqual(0);
        expect(row.ship_dow).toBeLessThanOrEqual(6);
        expect(new Date(row.quarter_start).getUTCMonth()).toBe(0);
        expect(row.avg_price).toBeGreaterThan(0);
        expect(row.max_discount_pct).toBeGreaterThanOrEqual(0);
        expect(row.min_qty).toBeGreaterThan(0);
        expect(row.avg_abs_discount).toBeGreaterThanOrEqual(0);
        expect(row.max_qty_sqrt).toBeGreaterThan(0);
        expect(row.max_orderkey_mod7).toBeGreaterThanOrEqual(0);
        expect(row.max_orderkey_mod7).toBeLessThanOrEqual(6);
        expect(row.avg_ship_to_receipt_days).toBeGreaterThan(0);
        expect(new Date(row.ship_plus_week) > new Date('1993-01-01')).toBe(true);
        expect(row.rows_scanned).toBeGreaterThan(0);
    }
});

test('string api.js functions work through execSql', async ({ page }) => {
    await page.goto('/tests/fixtures/index.html');
    await page.waitForFunction(() => window.__ready === true);

    const origin = new URL(page.url()).origin;
    const from = `${origin}/tests/data/lineitem-sf1.parquet`;

    const sql = `
        SELECT
            upper(l_shipinstruct) AS instruct,
            first(length(l_shipinstruct)) AS instruct_len,
            first(left(l_shipinstruct, 4)) AS instruct_prefix,
            first(right(l_shipinstruct, 4)) AS instruct_suffix,
            first(substring(l_shipinstruct, 1, 7)) AS instruct_substr,
            first(replace(l_shipinstruct, ' ', '_')) AS instruct_underscored,
            first(trim(lower(l_shipinstruct))) AS instruct_lower_trim,
            first(position('COD', l_shipinstruct)) AS instruct_cod_position,
            first(concat(l_returnflag, '-', l_linestatus)) AS flag_status,
            first(lpad(l_returnflag, 3, '*')) AS flag_lpad,
            count() AS rows_scanned
        FROM '${from}'
        WHERE l_orderkey <= 300
        GROUP BY upper(l_shipinstruct)
        ORDER BY instruct ASC
    `;

    const result = await page.evaluate((s) => window.runSql(s), sql);

    expect(Array.isArray(result)).toBe(true);
    expect(result.length).toBeGreaterThan(0);

    for (const row of result) {
        expect(row.instruct).toBe(row.instruct.toUpperCase());
        expect(row.instruct_len).toBeGreaterThan(0);
        expect(row.instruct_prefix.length).toBeLessThanOrEqual(4);
        expect(row.instruct.startsWith(row.instruct_prefix.toUpperCase())).toBe(true);
        expect(row.instruct_suffix.length).toBeLessThanOrEqual(4);
        expect(row.instruct_substr.length).toBeLessThanOrEqual(7);
        expect(row.instruct_underscored.includes(' ')).toBe(false);
        expect(row.instruct_lower_trim).toBe(row.instruct_lower_trim.trim().toLowerCase());
        expect(row.instruct_cod_position === 0 || row.instruct_cod_position > 0).toBe(true);
        expect(row.flag_status).toMatch(/^[A-Z]-[A-Z]$/);
        expect(row.flag_lpad.length).toBe(3);
        expect(row.rows_scanned).toBeGreaterThan(0);
    }
});

test('a function call in WHERE (e.g. year(...)) does not error with "Column _whereN not found"', async ({ page }) => {
    // Regression test: a WHERE clause on a non-trivial expression (not a
    // bare column) is hoisted into a synthetic calc[] column (`_where1`,
    // see compiler.rs::compile_where). Two bugs used to surface here:
    //  1. dvisor's fragment-skipping "check_feasible" pre-check looked
    //     `_where1` up in the file's physical metadata (it can't exist
    //     there, it's computed at runtime) and hard-errored instead of
    //     just not skipping the fragment.
    //  2. the calc body called the bare function name (`year(...)`)
    //     instead of `api.year(...)`, so it hit a JS ReferenceError.
    await page.goto('/tests/fixtures/index.html');
    await page.waitForFunction(() => window.__ready === true);

    const origin = new URL(page.url()).origin;
    const from = `${origin}/tests/data/lineitem-sf1.parquet`;

    const sql = `
        SELECT
            l_returnflag,
            sum(l_quantity)
        FROM '${from}'
        WHERE year(l_shipdate) = 1993 AND l_discount >= 0
        GROUP BY l_returnflag
    `;

    const result = await page.evaluate((s) => window.runSql(s), sql);

    expect(Array.isArray(result)).toBe(true);
    expect(result.length).toBeGreaterThan(0);
    for (const row of result) {
        expect(row.l_returnflag).toMatch(/^[A-Z]$/);
        expect(row.sum_l_quantity).toBeGreaterThan(0);
    }
});
