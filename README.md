# dvisor

dvisor (Data Visualization Optimized Retrieval) is a progressive query engine
that runs in the browser. It runs SQL queries against remote Parquet
files with no backend server, and streams partial results as they arrive.

Documentation, guides and live examples: **[dvisor.io](https://dvisor.io)**

## Usage

```html
<script type="module">
  import { Dvisor } from 'https://cdn.jsdelivr.net/npm/dvisor@1.0.0/dist/index.js';

  const dv = new Dvisor();
  const sql = `
    SELECT year(DTNASC) AS birth_year, count() AS births
    FROM 'https://storage.googleapis.com/dvisor/SINASC-2012-2021.parquet'
    GROUP BY 1
    ORDER BY birth_year`;

  // Each iteration yields the whole result so far, refined as more data is read.
  for await (const rows of dv.execSql(sql)) {
    console.log(rows);
  }
</script>
```

With a bundler, `npm install dvisor` and `import { Dvisor } from 'dvisor'`.
A Parquet file can be queried when its host allows CORS and HTTP range
requests, and a `.meta.json` file with its statistics sits next to it. The
[JavaScript Guide](https://dvisor.io/js-guide/) covers both.

## Repository

| Folder | Contents |
| --- | --- |
| `dvisor-back/` | The query engine, in Rust, compiled to WebAssembly. |
| `dvisor-web-lib/` | The JavaScript library: the `Dvisor` API and the Web Worker that runs the engine. |

## Building

Requires Rust, [wasm-pack](https://rustwasm.github.io/wasm-pack/) and Node.js.

```bash
cd dvisor-web-lib && npm install
cd ../dvisor-back && ./compile.sh   # engine -> dvisor-web-lib/pkg, then the library -> dvisor-web-lib/dist
```

Tests:

```bash
cd dvisor-back && cargo test
cd dvisor-web-lib && npx playwright install chromium && npm test
```

The first `npm test` downloads a 200 MB TPC-H test file. Publishing a
release: see [RELEASING.md](RELEASING.md).

## Documentation

- [JavaScript Guide](https://dvisor.io/js-guide/)
- [SQL Reference](https://dvisor.io/sql-reference/) and [Query DSL Reference](https://dvisor.io/query-dsl-reference/)
- [Progressive Charts with d3](https://dvisor.io/d3-guide/)
- [SQL Runner](https://dvisor.io/sql-runner/), to try queries in the browser
- [Reference for AI Agents](https://dvisor.io/ai-agents/)
- [Publications](https://dvisor.io/publications/)

## License

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
