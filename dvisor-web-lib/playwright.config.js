import { defineConfig } from '@playwright/test';

export default defineConfig({
    testDir: './tests',
    timeout: 60_000,
    webServer: {
        command: 'node tests/server.mjs',
        url: 'http://localhost:4173/tests/fixtures/index.html',
        reuseExistingServer: !process.env.CI,
    },
    use: {
        baseURL: 'http://localhost:4173',
    },
});
