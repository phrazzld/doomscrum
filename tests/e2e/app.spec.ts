import { expect, test } from '@playwright/test';

test('feed plays generated MP4 and records decisions', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'PRD Brainrot Swipe' })).toBeVisible();
  await expect(page.getByTestId('video-card')).toBeVisible();
  await expect(page.locator('video')).toBeVisible();

  await page.getByRole('button', { name: /Inspect/ }).click();
  await expect(page.getByTestId('source-panel')).toContainText('#');

  await page.getByRole('button', { name: /Needs spec/ }).click();
  await expect(page.getByText(/needs spec recorded/i)).toBeVisible();

  await page.getByRole('button', { name: /Launch Codex/ }).click();
  await expect(page.getByText(/Run packet created/i)).toBeVisible();
});
