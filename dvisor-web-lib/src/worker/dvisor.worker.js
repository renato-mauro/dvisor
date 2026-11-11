import init, { DvisorJsFacade } from '../../pkg/dvisor_back.js';
import * as api from "../shared/api.js";

globalThis.API = api;

// Populated by the `registerLocalFile` command below when a user picks a
// local parquet file (metadata-generation page): a blob: URL's size is
// already known synchronously from the File object at registration time,
// which matters because a HEAD request against a blob: URL is impossible
// (the browser hard-rejects any method but GET for that scheme) — see
// js_get_content_length below.
const localBlobSizes = new Map();

globalThis.js_fetch_range = function(url, start, end) {
    const xhr = new XMLHttpRequest();
    xhr.open("GET", url, false);
    xhr.responseType = "arraybuffer";
    xhr.setRequestHeader("Range", `bytes=${start}-${end}`);
    xhr.send();
    if (xhr.status !== 206 && xhr.status !== 200) {
        throw new Error(`HTTP ${xhr.status}`);
    }
    let data = new Uint8Array(xhr.response);
    return data;
};


globalThis.js_get_content_length = function (url) {
    if (localBlobSizes.has(url)) {
        return String(localBlobSizes.get(url));
    }
    const xhr = new XMLHttpRequest();
    xhr.open("HEAD", url, false);
    xhr.send();
    if (xhr.status < 200 || xhr.status >= 300) {
        throw new Error(`HTTP ${xhr.status}`);
    }
    const lenStr = xhr.getResponseHeader("Content-Length");
    if (!lenStr) {
        throw new Error("Missing Content-Length header");
    }
    return lenStr;
};

globalThis.js_fetch_string = (url) => {
    const xhr = new XMLHttpRequest();
    xhr.open('GET', url, false); 
    xhr.send(null);
    if (xhr.status < 200 || xhr.status >= 300) {
        throw new Error(`HTTP ${xhr.status}`);
    }
    const resp = xhr.responseText;
    return resp;
};

let dvisor = null;
let intervalHandler = null;
// Set when something fails that can't be attributed to a single query
// (typically a Rust panic, which traps the wasm instance and leaves it
// unusable): from then on the worker only answers, with this message.
let fault = null;
let apiMap = new Map();
for(let f in api) {
    apiMap.set(f.toLowerCase(),f);
}

globalThis.getApiFunctionName = function(name) {
    let transName = apiMap.get(name.toLowerCase());
    if(transName) return transName;
    throw new Error(`Function '${name}' not defined`);
} 

// Errors cross to the client as plain, non-empty strings: wasm-bindgen
// errors usually are strings already, but a JS Error (or DOMException) must
// be reduced to its message, and an empty one must not slip through as if
// there were no error at all.
function errorMessage(err) {
    if (err instanceof Error) {
        const msg = err.name && err.name !== 'Error' ? `${err.name}: ${err.message}` : err.message;
        return msg || err.name || 'Unknown error';
    }
    return String(err ?? '') || 'Unknown error';
}

function processNext() {
    try {
        let result = dvisor.next();
        if(result === false) {
            clearInterval(intervalHandler);
            intervalHandler = null;
        } else if(typeof(result)==="number") {
            globalThis.postMessage({ qid: result, result: null });
        } else {
            let buffers = [];
            if(result) {
                for(let k in result) {
                    let a = result[k];
                    if(a.buffer) {
                        buffers.push(a.buffer);
                    }
                }
            }
            globalThis.postMessage({ ok: true, result, qid: result.qid },buffers);
        }
    } catch(err) {
        if (err && err.qid !== undefined) {
            // A failed query, as DvisorJsFacade.next() reports it (having
            // already dropped it from its round-robin).
            globalThis.postMessage({ ok: false, error: errorMessage(err.source), qid: err.qid });
        } else {
            fault = errorMessage(err);
            clearInterval(intervalHandler);
            intervalHandler = null;
            globalThis.postMessage({ ok: false, fault: true, error: fault });
        }
    }
}

let commands = {

    config(mid, tid, args) {
        dvisor = new DvisorJsFacade(tid,args[0]);
        globalThis.postMessage({ ok: true, mid, tid });
    },

    // The reply carries no qid on purpose: the client routes anything with a
    // qid as query data, so this ack would never reach its mid callback.
    cancelQuery(mid, tid, args) {
        dvisor.cancel(args[0]);
        globalThis.postMessage({ ok: true, mid, tid });
    },

    exec(mid, tid, args) {
        if(typeof args[0] !== 'string') {
            throw 'args[0] for execute must be a string';
        }
        try {
            dvisor.exec(args[0],args[1]);
            if(!intervalHandler) {
                intervalHandler = setInterval(processNext,1);
            }
            globalThis.postMessage({ ok: true, mid, tid});
        }
        catch(err) {
            globalThis.postMessage({ ok: false, mid, tid, error: errorMessage(err) });
        }
    },

    createMetadata(mid, tid, args) {
        if(typeof args[0] !== 'string') {
            throw 'args[0] for execute must be a string';
        }
        let result = dvisor.create_metadata(args[0]);
        globalThis.postMessage({ ok: true, mid, tid, result });
    },

    // A local file never leaves the browser: the File object is structured-
    // cloned into this worker (a reference to the same underlying blob
    // storage, not a byte-for-byte copy) and turned into a blob: URL that's
    // only valid inside this worker's own scope — dvisor's existing
    // range-read code (js_fetch_range) then treats it exactly like a
    // remote URL, no server involved at any point.
    registerLocalFile(mid, tid, args) {
        const file = args[0];
        const url = URL.createObjectURL(file);
        localBlobSizes.set(url, file.size);
        globalThis.postMessage({ ok: true, mid, tid, url });
    },

    execSql(mid, tid, args) {
        if(typeof args[0] !== 'string') {
            throw 'args[0] for execSql must be a string';
        }
        try {
            let query = dvisor.compileSql(args[0]);
            dvisor.exec(query,args[1]);
            if(!intervalHandler) {
                intervalHandler = setInterval(processNext,1);
            }
            globalThis.postMessage({ ok: true, mid, tid});
        }
        catch(err) {
            globalThis.postMessage({ ok: false, mid, tid, error: errorMessage(err) });
        }
    }
};

let waitModule = null;

globalThis.onmessage = messageHandler;

async function messageHandler(e) {
    let {cmd, mid, tid, args} = e.data ?? {}
    if(!waitModule) {
        if(cmd !== "config") {
            globalThis.postMessage({ ok: false, mid, tid, error: "First message to worker must be 'config'" });
            return;
        }
        waitModule = init({ module_or_path: args[1] });
    }
    // Every message gets exactly one answer, failed initialization (the
    // .wasm missing, blocked by CORS or by a CSP without 'wasm-unsafe-eval')
    // and fault included: the client waits on one per message.
    try {
        await waitModule;
    } catch(err) {
        globalThis.postMessage({ ok: false, mid, tid, error: `Failed to initialize dvisor's WebAssembly module: ${errorMessage(err)}` });
        return;
    }
    if (fault) {
        globalThis.postMessage({ ok: false, mid, tid, error: fault });
        return;
    }
    let f = commands[cmd];
    if(f) {
        try {
            await f(mid,tid,args);
        } catch(e) {
            globalThis.postMessage({ ok: false, mid, tid, error: errorMessage(e) });
        }
    } else {
        globalThis.postMessage({ ok: false, mid, tid, error: 'unknown command' });
    }
}

