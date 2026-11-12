import { defineConfig, devices } from '@playwright/test';
import { SERVERS, type Variant } from './e2e/servers';

// End-to-end tests against the built binary (spec web-ui-shell). Prerequisites:
// `bun run build` and `cargo build -p sofarr-server` (see README.md).
const variants = Object.keys(SERVERS) as Variant[];

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
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: variants.map((variant) => ({
    command: `bun e2e/serve.mjs ${variant}`,
    url: `${SERVERS[variant]}/api/health`,
    reuseExistingServer: false,
    timeout: 60_000,
    stdout: 'ignore' as const,
    stderr: 'pipe' as const,
  })),
});
