// The exit criterion of lot 17b (specs/LOTS.md): an admin of a repository changes
// its configuration in the builder, sees the diff of the YAML and proposes it as
// a pull request, which CIHub opens without a push on `main`; the merge of the
// pull request ends the proposal.
import { expect, test } from "@playwright/test";

// The scheduler reads a new repository within 10 seconds, and the job of a
// proposal runs at once; the page reads again every 2 seconds while it waits.
const READ = { timeout: 30_000 };

test("an admin proposes a change of the configuration as a pull request", async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto("/sign-in");
  await page.getByRole("link", { name: /continue with github/i }).click();
  await page.getByRole("link", { name: /sign in as lin/i }).click();

  await expect(page).toHaveURL(/\/orgs\/umbrella/);

  // The first read of the setup of `shop` gives its file.
  await expect(async () => {
    await page.goto("/orgs/umbrella/repos/shop/config");
    await expect(page.getByRole("heading", { name: "Queue of main" })).toBeVisible({ timeout: 2_000 });
  }).toPass(READ);

  const size = page.getByRole("textbox", { name: "Batch size", exact: true });
  await size.fill("7");
  await size.press("Enter");
  const bar = page.getByRole("region", { name: "Pending changes" });

  await expect(bar).toContainText("+3 −0 pending changes to .cihub.yaml");

  await bar.getByRole("button", { name: "Propose as a pull request" }).click();
  const dialog = page.getByRole("dialog");

  await expect(dialog.getByText("size: 7")).toBeVisible();

  await dialog.getByRole("textbox").fill("Batches of 7");
  await dialog.getByRole("button", { name: "Open the pull request" }).click();
  const opened = page.getByText(/Pull request #\d+ changes this file\./);

  await expect(opened).toBeVisible(READ);

  const number = /#(\d+)/.exec((await opened.textContent()) ?? "")?.[1] ?? "";
  const merged = await page.request.get(`/sim/merge-pr?repo=shop&number=${number}`);
  expect(await merged.text()).toBe("merged");

  // The delivery of the merge ends the proposal. `shop` has no queue that reads its file at each push: the
  // read of its setup, which "Check again" asks for, gives the new file.
  const cookies = await page.context().cookies();
  const csrf = cookies.find((cookie) => cookie.name === "__Host-cihub_csrf")?.value ?? "";
  const sync = await page.request.post("/api/v1/orgs/umbrella/repos/shop/sync", {
    headers: { "X-CSRF-Token": csrf },
  });
  expect(sync.status()).toBe(202);

  await expect(async () => {
    await page.reload();
    await expect(opened).toHaveCount(0, { timeout: 2_000 });
    await expect(page.getByRole("textbox", { name: "Batch size", exact: true })).toHaveValue("7", {
      timeout: 2_000,
    });
  }).toPass(READ);
});
