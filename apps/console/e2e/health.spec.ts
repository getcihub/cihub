// The Health of a queue shows a
// merged PR and a failed one, from the facts that the notifier derives from the
// log.
import { type Page, expect, test } from "@playwright/test";

// The queue takes a moment to create its queue, to test a PR twice and to end it.
const READ_AGAIN = { timeout: 60_000 };

async function signInAsAda(page: Page) {
  await page.goto("/");
  await page.getByRole("link", { name: /continue with github/i }).click();
  await page.getByRole("link", { name: /sign in as ada/i }).click();
  await expect(page).toHaveURL(/\/orgs\/acme$/);
}

async function commentMerge(page: Page, number: number) {
  const commented = await page.request.get(`/sim/merge?repo=api&number=${String(number)}&login=ada`);
  expect(await commented.text()).toBe("commented");
}

test("the Health of a queue shows a merged PR and a failed one", async ({ page }) => {
  test.setTimeout(180_000);
  await signInAsAda(page);

  // #1 merges; the CI fails each run of #3, past the failure budget of one retry.
  await commentMerge(page, 1);
  const card = page.locator('a[href="/orgs/acme/repos/api/queues/main"]');
  await expect(card).toBeVisible(READ_AGAIN);
  await commentMerge(page, 3);
  await card.click();
  await page.getByRole("link", { name: "Health" }).click();
  await expect(page).toHaveURL(/\/orgs\/acme\/repos\/api\/queues\/main\?pane=health$/);

  // The facts come a moment after the log: the page reads them again until they are there.
  await expect(async () => {
    await page.reload();
    await expect(page.getByText("A required check failed")).toBeVisible({ timeout: 2_000 });
  }).toPass(READ_AGAIN);
  await expect(page.getByText("Many pull requests fail")).toBeVisible();
  await expect(page.getByText(/^1 of 2 pull requests failed$/)).toBeVisible();
  await expect(page.getByRole("row", { name: /^ci / })).toBeVisible();
});
