# dvisor

[![DOI](https://zenodo.org/badge/1454510738.svg)](https://doi.org/10.5281/zenodo.24148060)
[![npm](https://img.shields.io/npm/v/dvisor)](https://www.npmjs.com/package/dvisor)
[![License](https://img.shields.io/github/license/renato-mauro/dvisor)](LICENSE)
[![Docs](https://img.shields.io/badge/docs-dvisor.io-blue)](https://dvisor.io)


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
    FROM 'https://data.dvisor.io/data/datasus/SINASC-2012-2021.parquet'
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

The build runs on macOS, Linux and Windows. It needs a C toolchain, Rust
(installed with rustup) with its WebAssembly target,
[wasm-pack](https://wasm-bindgen.github.io/wasm-pack/) and Node.js 20 or
later.

**macOS**

```bash
xcode-select --install                                           # C toolchain and git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Rust
brew install node                                                # or the installer at nodejs.org
```

**Linux**, and Windows with WSL

```bash
sudo apt install build-essential curl git                        # Debian, Ubuntu; Fedora: sudo dnf install gcc curl git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Rust
```

Node.js: see [nodejs.org](https://nodejs.org/en/download); distribution
packages may be older than 20.

**Windows**

Install [Git for Windows](https://git-scm.com/downloads/win), which brings
Git Bash; Rust, running `rustup-init.exe` from [rustup.rs](https://rustup.rs)
and accepting the Visual Studio C++ Build Tools it offers; and Node.js, from
[nodejs.org](https://nodejs.org). Run the commands below in Git Bash.

**Then, on all three**, in a new terminal:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
git clone https://github.com/renato-mauro/dvisor.git
cd dvisor/dvisor-back && ./compile.sh
```

`compile.sh` checks these prerequisites first and says how to install any
that is missing. It builds the engine into `dvisor-web-lib/pkg` and the
library into `dvisor-web-lib/dist`. The first run also installs the
library's npm dependencies and downloads the Rust crates and the tools
wasm-pack uses, so it takes a few minutes.

Tests, from `dvisor-back`:

```bash
cargo test
cd ../dvisor-web-lib && npx playwright install chromium && npm test
```

On Linux, `npx playwright install --with-deps chromium` also installs the
system libraries Chromium needs. The first `npm test` downloads a 200 MB
TPC-H test file. Publishing a release: see [RELEASING.md](RELEASING.md).

## Documentation

- [JavaScript Guide](https://dvisor.io/js-guide/)
- [SQL Reference](https://dvisor.io/sql-reference/) and [Query DSL Reference](https://dvisor.io/query-dsl-reference/)
- [Progressive Charts with d3](https://dvisor.io/d3-guide/)
- [SQL Runner](https://dvisor.io/sql-runner/), to try queries in the browser
- [Reference for AI Agents](https://dvisor.io/ai-agents/)
- [Publications](https://dvisor.io/publications/)

## License

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
