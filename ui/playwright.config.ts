import { defineConfig, devices } from '@playwright/test';
import { e2ePassword } from './e2e/env.mjs';
import { SERVERS, type Variant } from './e2e/servers';

// End-to-end tests against the built binary (spec web-ui-shell). Prerequisites:
// `bun run build` and `cargo build -p sofarr-server` (see README.md).
const variants = Object.keys(SERVERS) as Variant[];
// Generated once here; the servers and the workers inherit it through the environment.
e2ePassword();

export default defineConfig({
  testDir: 'e2e',
  fullyParallel: true,
  forbidOnly: true,
  retries: 0,
  reporter: [['list']],
  use: {
    locale: 'de-DE',
    timezoneId: 'Europe/Berlin',
    trace: 'retain-on-failure',
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] }, testIgnore: /perf\.spec\.ts$/ },
    // The time measurement of web-ui-library AC-04 runs after all other tests, alone.
    {
      name: 'perf',
      use: { ...devices['Desktop Chrome'] },
      testMatch: /perf\.spec\.ts$/,
      dependencies: ['chromium'],
    },
  ],
  webServer: variants.map((variant) => ({
    command: `bun e2e/serve.mjs ${variant}`,
    url: `${SERVERS[variant]}/api/health`,
    reuseExistingServer: false,
    timeout: 180_000,
    stdout: 'ignore' as const,
    stderr: 'pipe' as const,
  })),
});
