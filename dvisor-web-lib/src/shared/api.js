export function dateTruncWeek(value) {
    if(!(value instanceof Date)) {
        throw new TypeError("Invalid date");
    }
    const d = new Date(value);
    d.setUTCDate(d.getUTCDate()-d.getUTCDay()%7);
    d.setUTCHours(0, 0, 0, 0);
    return d;
}

export function dateTruncMonth(value) {
    if(!(value instanceof Date)) {
        throw new TypeError("Invalid date");
    }
    const d = new Date(value);
    d.setUTCDate(1);
    d.setUTCHours(0, 0, 0, 0);
    return d;
}

export function dateTruncYear(value) {
    if(!(value instanceof Date)) {
        throw new TypeError("Invalid date");
    }
    return value.getUTCFullYear();
}

export function year(value) {
    return dateTruncYear(value);
}

export function dateTruncDay(value) {
    if(!(value instanceof Date)) {
        throw new TypeError("Invalid date");
    }
    const d = new Date(value);
    d.setUTCHours(0, 0, 0, 0);
    return d;
}

export function dateTruncQuarter(value) {
    if(!(value instanceof Date)) {
        throw new TypeError("Invalid date");
    }
    const d = new Date(value);
    d.setUTCMonth(Math.floor(d.getUTCMonth()/3)*3, 1);
    d.setUTCHours(0, 0, 0, 0);
    return d;
}

export function month(value) {
    if(!(value instanceof Date)) {
        throw new TypeError("Invalid date");
    }
    return value.getUTCMonth() + 1;
}

export function day(value) {
    if(!(value instanceof Date)) {
        throw new TypeError("Invalid date");
    }
    return value.getUTCDate();
}

export function quarter(value) {
    if(!(value instanceof Date)) {
        throw new TypeError("Invalid date");
    }
    return Math.floor(value.getUTCMonth()/3) + 1;
}

export function dayOfWeek(value) {
    if(!(value instanceof Date)) {
        throw new TypeError("Invalid date");
    }
    return value.getUTCDay();
}

export function dateDiffDays(date1, date2) {
    if(!(date1 instanceof Date) || !(date2 instanceof Date)) {
        throw new TypeError("Invalid date");
    }
    return Math.round((date2 - date1) / 86400000);
}

export function dateAddDays(value, n) {
    if(!(value instanceof Date)) {
        throw new TypeError("Invalid date");
    }
    const d = new Date(value);
    d.setUTCDate(d.getUTCDate() + n);
    return d;
}

export function upper(s) {
    if(typeof s !== 'string') {
        throw new TypeError("Invalid string");
    }
    return s.toUpperCase();
}

export function lower(s) {
    if(typeof s !== 'string') {
        throw new TypeError("Invalid string");
    }
    return s.toLowerCase();
}

export function trim(s) {
    if(typeof s !== 'string') {
        throw new TypeError("Invalid string");
    }
    return s.trim();
}

export function ltrim(s) {
    if(typeof s !== 'string') {
        throw new TypeError("Invalid string");
    }
    return s.trimStart();
}

export function rtrim(s) {
    if(typeof s !== 'string') {
        throw new TypeError("Invalid string");
    }
    return s.trimEnd();
}

export function length(s) {
    if(typeof s !== 'string') {
        throw new TypeError("Invalid string");
    }
    return s.length;
}

export function substring(s, start, len) {
    if(typeof s !== 'string') {
        throw new TypeError("Invalid string");
    }
    const from = Math.max(0, (start|0) - 1);
    return len === undefined ? s.substring(from) : s.substring(from, from + (len|0));
}

export function concat(...args) {
    return args.map(String).join('');
}

export function replace(s, search, repl) {
    if(typeof s !== 'string') {
        throw new TypeError("Invalid string");
    }
    return s.split(search).join(repl);
}

export function left(s, n) {
    if(typeof s !== 'string') {
        throw new TypeError("Invalid string");
    }
    return s.substring(0, n);
}

export function right(s, n) {
    if(typeof s !== 'string') {
        throw new TypeError("Invalid string");
    }
    return n <= 0 ? '' : s.substring(s.length - n);
}

export function lpad(s, len, pad = ' ') {
    s = String(s);
    if(s.length >= len) {
        return s.substring(0, len);
    }
    let p = '';
    while(p.length < len - s.length) {
        p += pad;
    }
    return p.substring(0, len - s.length) + s;
}

export function rpad(s, len, pad = ' ') {
    s = String(s);
    if(s.length >= len) {
        return s.substring(0, len);
    }
    let p = '';
    while(p.length < len - s.length) {
        p += pad;
    }
    return s + p.substring(0, len - s.length);
}

export function position(sub, s) {
    if(typeof sub !== 'string' || typeof s !== 'string') {
        throw new TypeError("Invalid string");
    }
    return s.indexOf(sub) + 1;
}

export function round(x, decimals = 0) {
    const f = Math.pow(10, decimals);
    return Math.round(x * f) / f;
}

export function ceil(x) {
    return Math.ceil(x);
}

export function floor(x) {
    return Math.floor(x);
}

export function trunc(x, decimals = 0) {
    const f = Math.pow(10, decimals);
    return Math.trunc(x * f) / f;
}

export function abs(x) {
    return Math.abs(x);
}

export function power(x, y) {
    return Math.pow(x, y);
}

export function sqrt(x) {
    return Math.sqrt(x);
}

export function sign(x) {
    return Math.sign(x);
}

export function mod(x, y) {
    return x % y;
}

// Fixed-width bucketing: returns the lower bound of the width-`width`
// bucket `x` falls into (e.g. bucket(3200, 500) === 3000). Same value as
// the floor(x/width)*width idiom, but as a named call so the SQL compiler
// can recognize it syntactically and rewrite a WHERE predicate on it into
// an equivalent range on the underlying column for row-group pruning (see
// dvisor-back/src/sql/compiler.rs's rewrite_for_pruning) — the arithmetic
// form still works but isn't recognized for that rewrite.
export function bucket(x, width) {
    return Math.floor(x / width) * width;
}

// Ternary conditional, since dvisor's SQL dialect has no CASE/WHEN: iif(cond,
// a, b) evaluates to `a` when `cond` is truthy, `b` otherwise. Typically used
// inside an aggregate to turn a predicate into a 0/1 indicator to sum, e.g.
// `sum(iif(PARTO = 2, 1, 0))` to count cesareans within a GROUP BY.
export function iif(cond, a, b) {
    return cond ? a : b;
}

