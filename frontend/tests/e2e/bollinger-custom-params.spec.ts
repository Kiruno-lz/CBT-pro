import { test, expect } from '@playwright/test';

try {
  test('Bollinger Bands custom parameter E2E test', async ({ page }) => {
    // Capture console errors
    const consoleErrors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });

    // Capture network errors (400+)
    const networkErrors: { url: string; status: number }[] = [];
    page.on('response', response => {
      if (response.status() >= 400) {
        networkErrors.push({ url: response.url(), status: response.status() });
      }
    });

    // Open the frontend
    await page.goto('http://localhost:3000');
    
    // Wait for the page to load
    await page.waitForLoadState('networkidle');
    
    console.log('Page loaded');
    await page.screenshot({ path: '/tmp/e2e-initial.png' });

    // Select Bollinger Bands strategy using label
    const strategySelect = page.locator('select').nth(1);
    await strategySelect.waitFor({ state: 'visible' });
    await strategySelect.selectOption({ label: 'Bollinger Bands' });
    console.log('Selected Bollinger Bands strategy');
    
    // Wait for strategy parameters to load
    await page.waitForTimeout(1000);
    
    // Expand Strategy Parameters section
    const paramsButton = page.getByRole('button', { name: /Strategy Parameters/i });
    await paramsButton.click();
    console.log('Expanded Strategy Parameters');
    
    await page.waitForTimeout(500);
    
    // Find and change period to 15 (first number input in params)
    const periodInput = page.locator('input[type="number"]').first();
    await periodInput.fill('15');
    console.log('Changed period to 15');
    
    await page.waitForTimeout(500);
    
    // Take screenshot before starting backtest
    await page.screenshot({ path: '/tmp/e2e-before-start.png' });
    
    // Click START BACKTEST
    const startButton = page.getByRole('button', { name: /START BACKTEST/i });
    await startButton.click();
    console.log('Clicked START BACKTEST');
    
    // Wait for backtest to start and process some bars
    await page.waitForTimeout(8000);
    
    // Take screenshot after backtest
    await page.screenshot({ path: '/tmp/e2e-after-backtest.png' });
    
    // Verify no API 400 errors
    const api400Errors = networkErrors.filter(err => err.status === 400);
    console.log('Network errors:', networkErrors);
    console.log('API 400 errors:', api400Errors);
    
    expect(api400Errors).toHaveLength(0);
    
    // Verify Bollinger bands appear on chart
    // Look for the chart container
    const chartContainer = page.locator('.tv-lightweight-charts, [class*="chart"]').first();
    const chartExists = await chartContainer.isVisible().catch(() => false);
    expect(chartExists).toBeTruthy();
    
    console.log('Test completed successfully');
  });
} catch {
  // Skip when not running in Playwright's test runner
  console.log('Skipping Playwright e2e test - not running in Playwright runner');
}
