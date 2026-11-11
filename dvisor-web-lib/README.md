# dvisor

dvisor is a progressive query engine for the browser. It runs SQL queries
against remote Parquet files with no backend server, and streams partial
results as they arrive. The engine is written in Rust and runs as
WebAssembly in a Web Worker.

Documentation, guides and live examples: **[dvisor.io](https://dvisor.io)**

## Install

From a CDN, with no build step:

```js
import { Dvisor } from 'https://cdn.jsdelivr.net/npm/dvisor@1.0.0/dist/index.js';
```

Keep the full path to `dist/index.js`: the library loads its worker and
`.wasm` file from that folder. jsDelivr's short forms (`.../dvisor@1.0.0`
without a path, or `/+esm`) serve the code from another location, where
these files are not found.

With a bundler (tested with Vite):

```bash
npm install dvisor
```

```js
import { Dvisor } from 'dvisor';
```

## Example

```js
const dv = new Dvisor();
const sql = `
  SELECT year(DTNASC) AS birth_year, count() AS births
  FROM 'https://data.dvisor.io/data/datasus/SINASC-2012-2021.parquet'
  GROUP BY 1
  ORDER BY birth_year`;

// Each iteration yields the whole result so far, refined as more data is read.
for await (const rows of dv.execSql(sql)) {
  console.log(rows);
}
```

A Parquet file can be queried when its host allows CORS and HTTP range
requests, and a `.meta.json` file with its statistics sits next to it. The
[JavaScript Guide](https://dvisor.io/js-guide/) covers both.

dvisor needs a browser with ES modules, Web Workers and WebAssembly. It does
not run in Node.js.

## License

Apache License 2.0. Source code: [github.com/renato-mauro/dvisor](https://github.com/renato-mauro/dvisor).
