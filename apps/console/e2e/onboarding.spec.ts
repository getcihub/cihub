// A person without an organization
// installs the App, comes back through the Setup URL, sees "Connecting" then the
// setup of each repository; a fixed rule and "Check again" make a repository
// ready, and the first `/cihub merge` starts its queue without a reload.
import { type Page, expect, test } from "@playwright/test";

// The scheduler reads a new repository within 10 seconds; the page reads again
// every 2 seconds.
const SETUP_READ = { timeout: 30_000 };

function repoRow(page: Page, name: string) {
  return page.locator("button[aria-expanded]").filter({ hasText: `initech/${name}` });
}

test("a person installs the App and starts the first queue", async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto("/");
  await page.getByRole("link", { name: /continue with github/i }).click();
  await page.getByRole("link", { name: /sign in as noor/i }).click();

  await expect(page.getByRole("heading", { name: "Welcome to CIHub, @noor" })).toBeVisible();

  await page.getByRole("link", { name: "Install the App on GitHub" }).click();
  await page.getByRole("link", { name: "Install on initech" }).click();
  // The Setup URL trusts no `installation_id`: it signs the person in again.
  await page.getByRole("link", { name: /sign in as noor/i }).click();

  await expect(page).toHaveURL(/\/orgs\/initech\?connecting=install$/);
  await expect(page.getByRole("heading", { name: "Connecting initech" })).toBeVisible();
  await expect(repoRow(page, "app")).toContainText("Ready", SETUP_READ);
  await expect(repoRow(page, "site")).toContainText("Cannot merge");
  await expect(repoRow(page, "docs")).toContainText("Invalid .cihub.yaml");
  await expect(page).toHaveURL(/\/orgs\/initech$/);

  await repoRow(page, "docs").click();

  await expect(page.getByText("batching.size is 50, out of range 2–20")).toBeVisible();
  await expect(page.getByText("batching: { size: 50 }")).toBeVisible();

  const exempt = await page.request.get("/sim/exempt?repo=site");
  expect(await exempt.text()).toBe("exempt");
  await repoRow(page, "site").click();
  await page.getByRole("button", { name: "Check again" }).click();

  await expect(repoRow(page, "site")).toContainText("Ready", SETUP_READ);

  const commented = await page.request.get("/sim/merge?repo=app&number=1&login=noor");
  expect(await commented.text()).toBe("commented");

  await expect(page.locator('a[href="/orgs/initech/repos/app/queues/main"]')).toBeVisible(SETUP_READ);
  await expect(repoRow(page, "app")).toHaveCount(0);
});
