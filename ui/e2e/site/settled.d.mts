import type { Page } from '@playwright/test';

export function ratingsSettled(
  page: Page,
  options?: { tries?: number; pauseMs?: number },
): Promise<void>;
