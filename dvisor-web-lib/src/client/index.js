const workerUrl = new URL('./dvisor.worker.js', import.meta.url);
const wasmUrl = new URL('./dvisor_back_bg.wasm', import.meta.url);

const workerSource = await fetch(workerUrl).then(r => {
    if (!r.ok) {
        throw new Error(`Failed to load ${workerUrl}: HTTP ${r.status}`);
    }
    return r.text();
});
const workerBlobUrl = URL.createObjectURL(new Blob([workerSource], { type: 'text/javascript' }));

let nextQid = 9299;
let mid = 0;
let callbackMap = new Map();
let nextResultHandlers = new Map();

function postMessage(workers, msg, callback) {
    let tid = 0;
    for(let w of workers) {
        msg.tid = tid++;
        msg.mid = mid++;
        callbackMap.set(msg.mid,callback);
        w.postMessage(msg);
    }
}

function postMessageFirst(workers, msg, callback) {
    let tid = 0;
    let w = workers[tid];
    msg.tid = tid;
    msg.mid = mid++;
    callbackMap.set(msg.mid,callback);
    w.postMessage(msg);
}

function messageHandler(e) {
    if(e.data.qid) {
        let handler = nextResultHandlers.get(e.data.qid);
        if(handler) {
            if(e.data.ok === false) {
                handler.onError(e.data.error);
                nextResultHandlers.delete(e.data.qid);
            } else {
                let result = e.data.result;
                if(!handler.onData(result)) {
                    nextResultHandlers.delete(e.data.qid);
                }
            }
        }
    } else {
        let emid = e.data.mid;
        let f = callbackMap.get(emid);
        if(f) {
            f(e);
            callbackMap.delete(emid);
            if(callbackMap.size == 0) {
                mid = 0;
            }
        }
    }
}

function prepareFirst(helper,result) {
    const fobj = helper.obj;
    let resp = [];
    if(result.length > 0) {
        let n = result[0].length;
        for(let i=0; i<n; i++) {
            resp.push(fobj(result,i));
        }
    }
    return resp;
}

function concat(helper,result,out) {
    const fobj = helper.obj;
    if(result.length > 0) {
        let n = result[0].length;
        for(let i=0; i<n; i++) {
            out.push(fobj(result,i));
        }
    }
}

function compare(a,b) {
    for(let i=0; i<a.length; i++) {
        if (a[i] > b[i]) return 1;
        if (a[i] < b[i]) return -1;
    }
    return 0;
}

function mergeAggregate(helper, last, result) {
    let { cmp, comb, obj } = helper;
    let resp = [];
    let na = last.length;
    let nb = result[0].length;
    let i = 0;
    let j = 0;
    while(i<na && j<nb) {
        let c = cmp(last,result,i,j,compare);
        if(c == 0) {
            comb(last,result,i,j);
            j++;
        }
        if(c <= 0) {
            resp.push(last[i]);
            i++;
        } else {
            resp.push(obj(result,j));
            j++;
        }
    }
    while(i<na) {
        resp.push(last[i]);
        i++;
    }
    while(j<nb) {
        resp.push(obj(result,j));
        j++;
    }
    return resp;
}

function mergeSorted(helper, last, result) {
    const { obj, sort } = helper;
    const cmp = sort(compare); 
    let resp = [];
    let na = last.length;
    let nb = result[0].length;
    let i = 0;
    let j = 0;
    let ob = null;
    while(i<na && j<nb) {
        if(ob == null) {
            ob = obj(result,j);
        }
        let c = cmp(last[i],ob);
        if(c == 0) {
            resp.push(last[i]);
            resp.push(ob);
            ob = null;
            i++;
            j++;
        }
        else if(c < 0) {
            resp.push(last[i]);
            i++;
        } else {
            resp.push(ob);
            ob = null;
            j++;
        }
    }
    while(i<na) {
        resp.push(last[i]);
        i++;
    }
    while(j<nb) {
        resp.push(obj(result,j));
        j++;
    }
    return resp;
}

function combiner(helper) {
    let { comb, cmp, sort } = helper;
    let cf;
    let last = null;
    if(comb && cmp && sort) {
        cf = combineAggregateAndSort;
    } else if(comb && cmp) {
        cf = combineAggregate;
    } else if(sort) {
        cf = combineSort;
    } else {
        cf = combineConcatenate;
    }

    function combineAggregateAndSort(result) {
        if(last == null) {
            last = prepareFirst(helper,result);
        } else {
            last = mergeAggregate(helper,last,result);
        }
        return last.toSorted(sort(compare));
    }

    function combineAggregate(result) {
        if(last == null) {
            last = prepareFirst(helper,result);
        } else {
            last = mergeAggregate(helper, last, result);
        }
        return last;
    }

    function combineSort(result) {
        if(last == null) {
            last = prepareFirst(helper,result);
        } else {
            last = mergeSorted(helper,last,result);
        }
        return last;
    }

    function combineConcatenate(result) {
        if(last == null) {
            last = prepareFirst(helper,result);
        } else {
            concat(helper,result,last);
        }
        return last;
    }

    return cf;
}

// Every failure a Dvisor instance reports is a DvisorError. `kind` names the
// phase it happened in:
//   'init'      the workers or the WebAssembly module could not be started;
//               the instance is unusable, and every request on it rejects;
//   'query'     a request was refused before producing any result (invalid
//               SQL, an unreachable file, unreadable metadata, ...);
//   'execution' a query failed while streaming results; the snapshots it
//               already delivered are valid but incomplete.
export class DvisorError extends Error {
    constructor(kind, message) {
        super(message);
        this.name = 'DvisorError';
        this.kind = kind;
    }
}

export class Dvisor {

    #tc;
    #workers = [];
    #ready;
    #rejectReady;
    #initialized = false;
    // Set once the instance can no longer serve requests (see #fail).
    #failure = null;
    // Failure callbacks of every request still waiting on the workers, so a
    // failure of the whole instance can reach them instead of leaving them
    // waiting forever.
    #active = new Set();

    constructor(tc=4) {
        this.#tc = tc;
        let resolveReady;
        let rejectReady;
        this.#ready = new Promise((resolve, reject) => {
            resolveReady = resolve;
            rejectReady = reject;
        });
        // A failed start is reported through every request anyway; this
        // only keeps it from also surfacing as an unhandled rejection when
        // nobody awaits `ready` itself.
        this.#ready.catch(() => {});
        this.#rejectReady = rejectReady;

        for(let i=0; i<tc; i++) {
            let w = new Worker(workerBlobUrl, { type: 'module' });
            w.addEventListener("message",messageHandler);
            w.addEventListener("message", e => {
                if(e.data.fault) {
                    this.#fail(new DvisorError('execution', e.data.error));
                }
            });
            w.addEventListener("error", e => {
                const kind = this.#initialized ? 'execution' : 'init';
                this.#fail(new DvisorError(kind, e.message || 'A dvisor worker failed to load or crashed'));
            });
            this.#workers.push(w);
        }

        let pending = tc;
        postMessage(this.#workers,{cmd:"config",args:[tc, wasmUrl.href]},e=>{
            if(e.data.error) {
                this.#fail(new DvisorError('init', e.data.error));
            } else if(--pending == 0 && !this.#failure) {
                this.#initialized = true;
                resolveReady();
            }
            return true;
        });
    }

    // Resolves once every worker has loaded the WebAssembly module; rejects
    // with a DvisorError of kind 'init' otherwise. Awaiting it is optional:
    // requests wait for it on their own.
    get ready() {
        return this.#ready;
    }

    // Puts the instance in a failed state: every pending and future request
    // rejects with `error`. Only the first failure counts.
    #fail(error) {
        if(this.#failure) return;
        this.#failure = error;
        this.#rejectReady(error);
        for(const fail of this.#active) {
            fail(error);
        }
    }

    exec(query) {
        return this.#run("exec", JSON.stringify(query));
    }

    execSql(sql) {
        return this.#run("execSql", sql);
    }

    #run(cmd, arg) {
        let qid = nextQid++;
        let done = false;
        let tc = this.#tc;
        let finished = 0;
        let nextItem = null;
        let pending = null;
        // A failure that arrived while nobody was awaiting next(): kept
        // until the next call delivers it.
        let error = null;
        let cf = null;

        const cancelInWorkers = () => {
            if(!this.#failure) {
                postMessage(this.#workers,{cmd: "cancelQuery", args: [qid]},()=>{});
            }
        };

        const settle = () => {
            done = true;
            nextResultHandlers.delete(qid);
            this.#active.delete(fail);
        };

        // The first failure ends the run; later ones (every worker reports
        // the same compile error, for instance) are ignored. The query is
        // cancelled in every worker, since a for-await whose next() rejects
        // never calls return(), and the error replaces any partial still
        // waiting to be read: a failed run's partial must not pass for a
        // valid one.
        const fail = (err) => {
            if(done) return;
            settle();
            cancelInWorkers();
            nextItem = null;
            if(pending) {
                pending.reject(err);
                pending = null;
            } else {
                error = err;
            }
        };

        const onData = (result) => {
            if(done) return false;
            if(result) {
                let value;
                // Merging runs code the engine generated for this query
                // (result.helper). If it can't run (most likely a CSP
                // without 'unsafe-eval'), the run must fail rather than end
                // as if it had produced no rows.
                try {
                    if(cf == null) {
                        cf = combiner(eval(result.helper));
                    }
                    value = cf(result);
                } catch(err) {
                    fail(new DvisorError('execution', `Could not merge partial results: ${err.message}`));
                    return false;
                }
                value.__tid = result.thread_id;
                if(pending) {
                    const { resolve } = pending;
                    pending = null; 
                    resolve({ value, done: false }); 
                } else {
                    nextItem = value;
                }
                return true;
            } else {
                finished++;
                if(finished == tc) {
                    settle();
                    if (pending) { 
                        pending.resolve({ value: undefined, done: true }); 
                        pending = null;
                    }
                    return false;
                }
            }
            return true;
        };
    
        const onError = (message) => fail(new DvisorError('execution', message));

        this.#active.add(fail);
        nextResultHandlers.set(qid,{onData,onError});

        this.#ready.then(() => {
            if(done) return;
            postMessage(this.#workers,{ cmd, args: [arg, qid] }, e => {
                if(e.data.error) {
                    fail(new DvisorError('query', e.data.error));
                }
            });
        }, fail);

        return {
            [Symbol.asyncIterator]() { 
                return this; 
            },

            next() {
                if (error) {
                    const err = error;
                    error = null;
                    return Promise.reject(err);
                }
                if (nextItem) {
                    let value = nextItem;
                    nextItem = null;
                    return Promise.resolve({ value, done: false });
                }
                if (done) {
                    return Promise.resolve({ value: undefined, done: true });
                }
                return new Promise((resolve, reject) => { pending = { resolve, reject }; });
            },

            async return() {
                if(!done) {
                    settle();
                    cancelInWorkers();
                }
                nextItem = null;
                error = null;
                if(pending) {
                    pending.resolve({ value: undefined, done: true });
                    pending = null;
                }
                return { value: undefined, done: true };
            }
        };
    }

    // A one-shot command answered by worker 0 (the same one every call of
    // this kind targets, which registerLocalFile relies on). Resolves with
    // the worker's reply.
    #request(cmd, arg) {
        return new Promise((resolve, reject) => {
            const fail = (err) => {
                this.#active.delete(fail);
                reject(err);
            };
            this.#active.add(fail);
            this.#ready.then(() => {
                postMessageFirst(this.#workers,{ cmd, args: [arg] }, e => {
                    if(!this.#active.delete(fail)) return;
                    if(e.data.ok) {
                        resolve(e.data);
                    } else {
                        reject(new DvisorError('query', e.data.error));
                    }
                });
            }, fail);
        });
    }

    async createMetadata(name) {
        const reply = await this.#request("createMetadata", name);
        return JSON.parse(reply.result);
    }

    // Hands a local File to worker 0 (the same one createMetadata's
    // postMessageFirst always targets) and gets back a blob: URL valid only
    // inside that worker's own scope. No network/server round-trip: the
    // File is structured-cloned (a handle to the same underlying storage,
    // not a byte copy) into the worker, which registers it and answers with
    // the URL to pass straight into createMetadata(url).
    async registerLocalFile(file) {
        const reply = await this.#request("registerLocalFile", file);
        return reply.url;
    }
}
