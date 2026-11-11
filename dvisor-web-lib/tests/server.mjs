import { createServer } from 'node:http';
import { createReadStream, statSync } from 'node:fs';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const port = process.env.PORT ? Number(process.env.PORT) : 4173;

const MIME = {
    '.js': 'text/javascript',
    '.mjs': 'text/javascript',
    '.wasm': 'application/wasm',
    '.json': 'application/json',
    '.html': 'text/html',
    '.parquet': 'application/octet-stream',
};

// Fault injection for tests/dvisor-errors.spec.js. Each virtual directory
// serves real files under a path where something is deliberately broken,
// so a test can provoke a failure at one exact stage of a query:
//   /tests/no-wasm/      the library and worker, but no .wasm next to them
//                        (initialization fails);
//   /tests/no-data/      a .meta.json, but no .parquet next to it (planning
//                        fails: the query can't even be opened);
//   /tests/broken-data/  the real .parquet, but every range read outside its
//                        last BROKEN_DATA_TAIL bytes answers 503 (opening,
//                        which only reads the footer, succeeds; the first
//                        column-chunk read during execution fails).
const ALIASES = [
    [/^\/tests\/no-wasm\/(index\.js|dvisor\.worker\.js)$/, '/dist/$1'],
    [/^\/tests\/(?:no-data|broken-data)\/([^/]+\.meta\.json)$/, '/tests/data/$1'],
    [/^\/tests\/broken-data\/([^/]+\.parquet)$/, '/tests/data/$1'],
];
const BROKEN_DATA_TAIL = 64 * 1024;

function resolvePath(url) {
    const decoded = decodeURIComponent(url.split('?')[0]);
    let safe = normalize(decoded).replace(/^(\.\.[/\\])+/, '');
    for (const [pattern, target] of ALIASES) {
        if (pattern.test(safe)) {
            safe = safe.replace(pattern, target);
            break;
        }
    }
    return join(root, safe);
}

const server = createServer((req, res) => {
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Accept-Ranges', 'bytes');

    const filePath = resolvePath(req.url === '/' ? '/tests/fixtures/index.html' : req.url);

    let stat;
    try {
        stat = statSync(filePath);
    } catch {
        res.writeHead(404).end('Not found');
        return;
    }

    const contentType = MIME[extname(filePath)] || 'application/octet-stream';

    if (req.method === 'HEAD') {
        res.writeHead(200, { 'Content-Length': stat.size, 'Content-Type': contentType });
        res.end();
        return;
    }

    const range = req.headers.range;
    if (range) {
        const match = /^bytes=(\d+)-(\d*)$/.exec(range);
        if (!match) {
            res.writeHead(416, { 'Content-Range': `bytes */${stat.size}` }).end();
            return;
        }
        const start = Number(match[1]);
        const end = match[2] ? Number(match[2]) : stat.size - 1;
        if (req.url.startsWith('/tests/broken-data/') && start < stat.size - BROKEN_DATA_TAIL) {
            res.writeHead(503).end('Injected failure');
            return;
        }
        res.writeHead(206, {
            'Content-Range': `bytes ${start}-${end}/${stat.size}`,
            'Content-Length': end - start + 1,
            'Content-Type': contentType,
        });
        createReadStream(filePath, { start, end }).pipe(res);
        return;
    }

    res.writeHead(200, { 'Content-Length': stat.size, 'Content-Type': contentType });
    createReadStream(filePath).pipe(res);
});

server.listen(port, () => {
    console.log(`test server listening on http://localhost:${port}`);
});
