// The exit criterion of lot 10c (specs/LOTS.md): a person creates a queue by
// the first `/cihub merge`, submits a PR from the dashboard and follows it in
// "Your pull requests" until the merge; a member pauses nothing; the comment
// of the PR links to its page.
import { type Page, expect, test } from "@playwright/test";

// The queue takes a moment to create its queue and to test a PR.
const READ_AGAIN = { timeout: 30_000 };

async function signInAsAda(page: Page) {
  await page.goto("/");
  await page.getByRole("link", { name: /continue with github/i }).click();
  await page.getByRole("link", { name: /sign in as ada/i }).click();
  await expect(page).toHaveURL(/\/orgs\/acme$/);
}

test("a person submits a PR from the dashboard and follows it until the merge", async ({ page }) => {
  test.setTimeout(120_000);
  await signInAsAda(page);

  // The first `/cihub merge` on a PR of acme/api creates the queue of its branch.
  const commented = await page.request.get("/sim/merge?repo=api&number=1&login=ada");
  expect(await commented.text()).toBe("commented");

  const card = page.getByRole("link", { name: /acme\/api\s*main/ });
  await expect(card).toBeVisible(READ_AGAIN);
  await card.click();

  await page.getByRole("button", { name: "Submit a pull request" }).click();
  const dialog = page.getByRole("dialog", { name: "Submit a pull request" });
  await dialog.getByRole("textbox", { name: /Pull request number/ }).fill("2");
  await dialog.getByRole("button", { name: "Submit" }).click();

  await expect(page.getByText(/^#2 sent/)).toBeVisible();

  await page.goto("/orgs/acme");
  const mine = page.getByRole("region", { name: "Your pull requests" });

  await expect(mine).toContainText("#2 Retry the webhook sweep", READ_AGAIN);
  await expect(mine).toContainText("merged into main", { timeout: 60_000 });

  const comment = await (await page.request.get("/sim/comment?repo=api&number=2")).text();
  expect(comment).toContain("[Open in CIHub](http://127.0.0.1:18090/orgs/acme/repos/api/queues/main/prs/2)");
});

test("a member pauses nothing", async ({ page }) => {
  await signInAsAda(page);
  await page.goto("/orgs/acme/repos/web/queues/main");

  const pause = page.getByRole("button", { name: "Pause" });

  await expect(pause).toBeDisabled();
  await expect(pause).toHaveAttribute("title", "Needs the admin role");
});
