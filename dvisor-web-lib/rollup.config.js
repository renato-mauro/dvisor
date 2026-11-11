import { readFileSync } from 'node:fs';
import terser from '@rollup/plugin-terser';

function copyWasm() {
    return {
        name: 'copy-wasm',
        generateBundle() {
            this.emitFile({
                type: 'asset',
                fileName: 'dvisor_back_bg.wasm',
                source: readFileSync(new URL('./pkg/dvisor_back_bg.wasm', import.meta.url))
            });
        }
    };
}

export default [{
    onwarn: function (warning, warn) {
        if (warning.code === 'EVAL') return;
        warn(warning);
    },
    input: {
        index: 'src/client/index.js',
        'dvisor.worker': 'src/worker/dvisor.worker.js'
    },
    output: {
        dir: 'dist',
        format: 'es',
        entryFileNames: '[name].js',
        sourcemap: true
    },
    plugins: [
        copyWasm(),
        terser()
    ]
}];
