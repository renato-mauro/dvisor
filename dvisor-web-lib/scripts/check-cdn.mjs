// Checks a published version from the public CDNs that mirror npm: imports
// it in Chromium from a page of another origin, as a site would, and runs a
// query, which needs the worker and the .wasm loaded from beside index.js.
//
//     npm run check:cdn                  this package.json's version, on jsDelivr and unpkg
//     npm run check:cdn -- 1.0.1         another version
//     npm run check:cdn -- https://host/path/to/dist   any folder holding index.js
import { readFileSync } from 'node:fs';
import { chromium } from '@playwright/test';

const { name, version } = JSON.parse(readFileSync(new URL('../package.json', import.meta.url)));
const arg = process.argv[2];
const bases = arg?.startsWith('http')
    ? [arg.replace(/\/$/, '')]
    : [`https://cdn.jsdelivr.net/npm/${name}@${arg || version}/dist`, `https://unpkg.com/${name}@${arg || version}/dist`];

const SQL = `SELECT count() AS n FROM 'https://data.dvisor.io/data/tpch/lineitem-sf1.parquet'`;
const PAGE = 'http://cdn-check.test/';

const browser = await chromium.launch();
let failures = 0;
for (const base of bases) {
    const page = await browser.newPage();
    await page.route(PAGE, (route) => route.fulfill({ contentType: 'text/html', body: '<!doctype html><title>check</title>' }));
    await page.goto(PAGE);
    const result = await page.evaluate(async ({ url, sql }) => {
        try {
            const { Dvisor } = await import(url);
            let rows = [];
            for await (const snapshot of new Dvisor().execSql(sql)) rows = snapshot.map((r) => ({ ...r }));
            return { ok: rows.length === 1 && rows[0].n > 0, rows };
        } catch (e) {
            return { ok: false, error: String(e?.message || e) };
        }
    }, { url: `${base}/index.js`, sql: SQL });
    if (!result.ok) failures++;
    console.log(`${result.ok ? 'ok  ' : 'FAIL'} ${base}/index.js — ${JSON.stringify(result.rows ?? result.error)}`);
    await page.close();
}
await browser.close();
process.exit(failures ? 1 : 0);
