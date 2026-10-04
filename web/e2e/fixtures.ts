import { test as base } from "@playwright/test";

export { expect } from "@playwright/test";

export const test = base.extend({
  page: async ({ page }, use) => {
    // The tests run without the cihub server: each test mocks the API routes it reads.
    await page.route("**/api/**", (route) => route.fulfill({ status: 503 }));
    await use(page);
  },
});
