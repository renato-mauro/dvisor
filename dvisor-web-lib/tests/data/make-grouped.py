"""Writes tests/data/grouped.parquet, the fixture of tests/dvisor-grouping.spec.js.

Laid out like the SINASC file: one row group per (region, year), sorted by
region and then year, so each fragment holds only some of the groups a query
produces. That is what exposes merge bugs that lineitem hides: there, almost
every row group holds every group.

Run with the dvisor-pytools environment, then generate the metadata:

    ../dvisor-pytools/.venv/bin/python tests/data/make-grouped.py
    ../dvisor-pytools/.venv/bin/dvisor-pytools meta tests/data/grouped.parquet

The data is pseudo-random with a fixed seed, so the file is reproducible.
"""
import datetime
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq

REGIONS = [11, 12, 21, 22, 31, 32]
YEARS = [2018, 2019, 2020, 2021]
# Flags present in each region: some fragments lack some groups.
FLAGS = {11: ['A', 'B'], 12: ['A', 'B', 'C'], 21: ['B', 'C'], 22: ['A', 'B', 'C'], 31: ['A', 'C'], 32: ['C']}

SCHEMA = pa.schema([
    ('dt', pa.date32()),
    ('region', pa.int32()),
    ('sex', pa.int32()),
    ('flag', pa.string()),
    ('weight', pa.int32()),
])


def row_group(rng, k, region, year):
    n = 150 * (1 + k % 5)
    start = datetime.date(year, 1, 1)
    days = (datetime.date(year, 12, 31) - start).days + 1
    dt = [start + datetime.timedelta(days=int(d)) for d in rng.integers(0, days, n)]
    # Sex 0 ("unknown") appears only in every third row group.
    sex_values, sex_p = ([0, 1, 2], [0.05, 0.5, 0.45]) if k % 3 == 0 else ([1, 2], [0.52, 0.48])
    return pa.table({
        'dt': pa.array(dt, pa.date32()),
        'region': pa.array(np.full(n, region), pa.int32()),
        'sex': pa.array(rng.choice(sex_values, n, p=sex_p), pa.int32()),
        'flag': pa.array(rng.choice(FLAGS[region], n), pa.string()),
        'weight': pa.array(rng.integers(500, 5000, n), pa.int32()),
    }, schema=SCHEMA)


def main():
    rng = np.random.default_rng(20261031)
    out = Path(__file__).with_name('grouped.parquet')
    with pq.ParquetWriter(out, SCHEMA, compression='snappy') as writer:
        k = 0
        for region in REGIONS:
            for year in YEARS:
                table = row_group(rng, k, region, year)
                writer.write_table(table, row_group_size=table.num_rows)
                k += 1
    print(f'wrote {out} ({k} row groups)')


if __name__ == '__main__':
    main()
