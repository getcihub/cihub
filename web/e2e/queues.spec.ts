// The exit criterion of lot 10b (specs/LOTS.md): the cards of the queues and the
// page of a queue, with the PRs that `ada` submitted by comment on the
// simulator, their title, author and position, and the runs of the graph.
import { type Page, expect, test } from "@playwright/test";

// The queue tests its PRs a few seconds after the start of the server; the page
// reads it again every 10 seconds.
const QUEUE_SETTLES = { timeout: 30_000 };

async function signInAsAda(page: Page) {
  await page.goto("/");
  await page.getByRole("link", { name: /continue with github/i }).click();
  await page.getByRole("link", { name: /sign in as ada/i }).click();
  await expect(page).toHaveURL(/\/orgs\/acme$/);
}

test("the repositories and their queues lead to the page of a queue", async ({ page }) => {
  await signInAsAda(page);

  const card = page.locator('a[href="/orgs/acme/repos/web/queues/main"]');

  await expect(card).toContainText("3 in the queue", QUEUE_SETTLES);
  await expect(card).toContainText("2 testing");
  await expect(card).toContainText("Running");

  await card.click();

  await expect(page).toHaveURL(/\/orgs\/acme\/repos\/web\/queues\/main$/);
  await expect(page.getByRole("heading", { level: 1, name: /main/ })).toBeVisible();
});

test("the page of a queue shows its PRs in their order and the runs that test them", async ({ page }) => {
  await signInAsAda(page);
  await page.goto("/orgs/acme/repos/web/queues/main");

  const table = page.getByRole("table");
  const rows = table.getByRole("row");
  const positions = table.getByTestId("queue-position");

  await expect(rows).toHaveCount(4, QUEUE_SETTLES);
  await expect(positions).toHaveText(["1", "2", "–"]);
  await expect(rows.nth(3)).toContainText("Update the install guide");
  await expect(rows.nth(3)).toContainText("#3 · alex");
  await expect(rows.nth(3)).toContainText("Waiting");
  await expect(table).toContainText("Export invoices as CSV");
  await expect(table).toContainText("#1 · marie");
  await expect(table).toContainText("Fix the login redirect");
  await expect(table).toContainText("#2 · lea");

  // The run of the PR in the first position merges first: it is Run 1.
  const first =
    (await rows
      .nth(1)
      .getByText(/^#\d+ · /)
      .textContent()) ?? "";
  const second =
    (await rows
      .nth(2)
      .getByText(/^#\d+ · /)
      .textContent()) ?? "";
  const number = (text: string) => text.split(" ")[0] ?? "";

  await expect(page.getByRole("group", { name: "Run 1" })).toContainText(number(first));
  await expect(page.getByRole("group", { name: "Run 1" })).toContainText("Testing · attempt 1");
  await expect(page.getByRole("group", { name: "Run 2" })).toContainText(number(second));
  await expect(page.getByRole("group", { name: "Waiting" })).toContainText("#3");
  await expect(page.getByRole("group", { name: "Branch main" })).toBeVisible();
  await expect(page.getByText("Pipelined")).toBeVisible();
});

test("the tab of the page of a queue stays in its address", async ({ page }) => {
  await signInAsAda(page);
  await page.goto("/orgs/acme/repos/web/queues/main");

  const merged = page.getByRole("link", { name: /^Merged/ });
  await merged.click();

  await expect(page).toHaveURL(/\/orgs\/acme\/repos\/web\/queues\/main\?tab=merged$/);
  await expect(page.getByText("Nothing merged in the last 24 hours.")).toBeVisible();

  await page.reload();

  await expect(merged).toHaveAttribute("aria-current", "page");

  await page.getByRole("link", { name: /^In the queue/ }).click();

  await expect(page).toHaveURL(/\/orgs\/acme\/repos\/web\/queues\/main$/);
  await expect(page.getByRole("table")).toBeVisible();
});
