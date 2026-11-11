// Queries for tests/dvisor-grouping.spec.js. Each runs in dvisor (SQL or the
// JSON DSL) and, as an independent oracle, in DuckDB (see make-expected.mjs).
// `keys` names the group columns; `order` the order the rows must come in.
// `{FROM}` stands for the file: tests/data/grouped.parquet (one row group per
// region and year, see tests/data/make-grouped.py) or lineitem.

export const cases = [
    {
        name: 'two plain group columns, no ORDER BY',
        sql: 'SELECT region, sex, count() AS n FROM {FROM}',
        duckdb: 'SELECT region, sex, count(*) AS n FROM {FROM} GROUP BY 1, 2',
        keys: ['region', 'sex'],
    },
    {
        name: 'two group columns, ORDER BY an aggregate',
        sql: 'SELECT region, sex, count() AS n FROM {FROM} ORDER BY n DESC',
        duckdb: 'SELECT region, sex, count(*) AS n FROM {FROM} GROUP BY 1, 2',
        keys: ['region', 'sex'],
        order: [['n', 'desc']],
    },
    {
        name: 'ORDER BY only the first group column',
        sql: 'SELECT region, sex, count() AS n FROM {FROM} ORDER BY region',
        duckdb: 'SELECT region, sex, count(*) AS n FROM {FROM} GROUP BY 1, 2',
        keys: ['region', 'sex'],
        order: [['region', 'asc']],
    },
    {
        name: 'ORDER BY the group columns in reverse order',
        sql: 'SELECT region, sex, count() AS n FROM {FROM} ORDER BY sex, region',
        duckdb: 'SELECT region, sex, count(*) AS n FROM {FROM} GROUP BY 1, 2',
        keys: ['region', 'sex'],
        order: [['sex', 'asc'], ['region', 'asc']],
    },
    {
        name: 'renamed plain group column, not the first',
        sql: 'SELECT year(dt) AS yr, sex AS gender, count() AS n FROM {FROM} GROUP BY 1, 2 ORDER BY yr, gender',
        duckdb: 'SELECT year(dt) AS yr, sex AS gender, count(*) AS n FROM {FROM} GROUP BY 1, 2',
        keys: ['yr', 'gender'],
        order: [['yr', 'asc'], ['gender', 'asc']],
    },
    {
        name: 'single renamed group column, not the first',
        sql: 'SELECT count() AS n, sex AS gender FROM {FROM} ORDER BY gender',
        duckdb: 'SELECT count(*) AS n, sex AS gender FROM {FROM} GROUP BY sex',
        keys: ['gender'],
        order: [['gender', 'asc']],
    },
    {
        name: 'calculated and plain group columns, DESC',
        sql: 'SELECT year(dt) AS yr, flag, avg(weight) AS w, count() AS n FROM {FROM} GROUP BY 1, 2 ORDER BY yr DESC, flag',
        duckdb: 'SELECT year(dt) AS yr, flag, avg(weight) AS w, count(*) AS n FROM {FROM} GROUP BY 1, 2',
        keys: ['yr', 'flag'],
        order: [['yr', 'desc'], ['flag', 'asc']],
    },
    {
        name: 'three group columns',
        sql: 'SELECT region, flag, sex, count() AS n FROM {FROM}',
        duckdb: 'SELECT region, flag, sex, count(*) AS n FROM {FROM} GROUP BY 1, 2, 3',
        keys: ['region', 'flag', 'sex'],
    },
    {
        // weight is stored in 16 bits; its sums used to be computed in the
        // storage type too, and overflowed.
        name: 'sum of a narrow integer column',
        sql: 'SELECT region, sum(weight) AS total, count() AS n FROM {FROM} ORDER BY region',
        duckdb: 'SELECT region, CAST(sum(weight) AS BIGINT) AS total, count(*) AS n FROM {FROM} GROUP BY region',
        keys: ['region'],
        order: [['region', 'asc']],
    },
    {
        name: 'sum and average of an integer column, no group columns',
        sql: 'SELECT sum(weight) AS total, avg(weight) AS w, count() AS n FROM {FROM}',
        duckdb: 'SELECT CAST(sum(weight) AS BIGINT) AS total, avg(weight) AS w, count(*) AS n FROM {FROM}',
        keys: [],
    },
    {
        name: 'min and max of a date',
        sql: 'SELECT min(dt) AS d0, max(dt) AS d1, count() AS n FROM {FROM}',
        duckdb: 'SELECT min(dt) AS d0, max(dt) AS d1, count(*) AS n FROM {FROM}',
        keys: [],
    },
    {
        name: 'min and max of a date, by group',
        sql: 'SELECT region, min(dt) AS d0, max(dt) AS d1, count() AS n FROM {FROM} ORDER BY region',
        duckdb: 'SELECT region, min(dt) AS d0, max(dt) AS d1, count(*) AS n FROM {FROM} GROUP BY region',
        keys: ['region'],
        order: [['region', 'asc']],
    },
    {
        name: 'JSON DSL: two group columns, sorted by an aggregate',
        dsl: {
            from: '{FROM}',
            project: [{ expr: 'region' }, { expr: 'flag' }, { aggregate: 'count', alias: 'n' }],
            sort: [{ expr: 'n', direction: 'DESC' }],
        },
        duckdb: 'SELECT region, flag, count(*) AS n FROM {FROM} GROUP BY 1, 2',
        keys: ['region', 'flag'],
        order: [['n', 'desc']],
    },
    {
        name: 'lineitem: two plain group columns, no ORDER BY',
        file: 'lineitem',
        sql: 'SELECT l_returnflag, l_linestatus, count() AS n FROM {FROM}',
        duckdb: 'SELECT l_returnflag, l_linestatus, count(*) AS n FROM {FROM} GROUP BY 1, 2',
        keys: ['l_returnflag', 'l_linestatus'],
    },
    {
        name: 'lineitem: two group columns, ORDER BY an aggregate',
        file: 'lineitem',
        sql: 'SELECT l_returnflag, l_linestatus, count() AS n FROM {FROM} ORDER BY n DESC',
        duckdb: 'SELECT l_returnflag, l_linestatus, count(*) AS n FROM {FROM} GROUP BY 1, 2',
        keys: ['l_returnflag', 'l_linestatus'],
        order: [['n', 'desc']],
    },
    {
        // Decimals are read into f32, each the nearest to its value; sums and
        // averages of them are computed in f64.
        name: 'lineitem: sums and averages of decimal columns',
        file: 'lineitem',
        sql: 'SELECT l_returnflag, l_linestatus, sum(l_quantity) AS qty, sum(l_extendedprice) AS price, avg(l_discount) AS disc, count() AS n FROM {FROM} ORDER BY l_returnflag, l_linestatus',
        duckdb: 'SELECT l_returnflag, l_linestatus, CAST(sum(l_quantity) AS DOUBLE) AS qty, CAST(sum(l_extendedprice) AS DOUBLE) AS price, CAST(avg(l_discount) AS DOUBLE) AS disc, count(*) AS n FROM {FROM} GROUP BY 1, 2',
        keys: ['l_returnflag', 'l_linestatus'],
        order: [['l_returnflag', 'asc'], ['l_linestatus', 'asc']],
    },
    {
        // 0.05 used to be read about 2e-8 low, below the literal, so no row
        // matched.
        name: 'lineitem: equality filter on a decimal column',
        file: 'lineitem',
        sql: 'SELECT l_returnflag, count() AS n FROM {FROM} WHERE l_discount = 0.05 ORDER BY l_returnflag',
        duckdb: 'SELECT l_returnflag, count(*) AS n FROM {FROM} WHERE l_discount = 0.05 GROUP BY 1',
        keys: ['l_returnflag'],
        order: [['l_returnflag', 'asc']],
    },
];

export const files = {
    grouped: 'tests/data/grouped.parquet',
    lineitem: 'tests/data/lineitem-sf1.parquet',
};
