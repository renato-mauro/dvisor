// Downloads the TPC-H SF=1 lineitem file most tests query, into tests/data,
// unless it is already there. At over 200 MB, it is too large for git.
import { createWriteStream, existsSync, renameSync } from 'node:fs';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';

const SOURCE = 'https://data.dvisor.io/data/tpch';
const FILES = ['lineitem-sf1.parquet', 'lineitem-sf1.meta.json'];
const dir = new URL('../tests/data/', import.meta.url);

for (const name of FILES) {
    const target = new URL(name, dir);
    if (existsSync(target)) continue;
    console.log(`Downloading ${SOURCE}/${name} into tests/data/ ...`);
    const response = await fetch(`${SOURCE}/${name}`);
    if (!response.ok) throw new Error(`${SOURCE}/${name}: HTTP ${response.status}`);
    // Written beside the target first, so an interrupted download never
    // leaves a truncated file that a later run would take as complete.
    const partial = new URL(`${name}.part`, dir);
    await pipeline(Readable.fromWeb(response.body), createWriteStream(partial));
    renameSync(partial, target);
}
