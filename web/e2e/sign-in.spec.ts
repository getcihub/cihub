// The exit criterion of lot 10a (specs/LOTS.md): the sign-in through the
// simulator, the role of a member, the isolation of the organizations, a
// refused sign-in and a person without an organization.
import { type Page, expect, test } from "@playwright/test";

/** Signs in through the page of the simulator, as `choice` ("Sign in as ada", "Refuse"). */
async function signIn(page: Page, choice: RegExp) {
  await page.goto("/");
  await expect(page).toHaveURL(/\/sign-in/);
  await page.getByRole("link", { name: /continue with github/i }).click();
  await page.getByRole("link", { name: choice }).click();
}

async function csrfToken(page: Page): Promise<string> {
  const cookies = await page.context().cookies();
  return cookies.find((cookie) => cookie.name === "__Host-cihub_csrf")?.value ?? "";
}

test("a person signs in and sees their organization", async ({ page }) => {
  await signIn(page, /sign in as ada/i);

  await expect(page).toHaveURL(/\/orgs\/acme$/);
  await expect(page.getByRole("button", { name: "Organization: acme" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Queues" })).toBeVisible();
});

test("a member cannot change the state of a queue", async ({ page }) => {
  await signIn(page, /sign in as ada/i);
  await expect(page).toHaveURL(/\/orgs\/acme$/);

  const paused = await page.request.patch("/api/v1/orgs/acme/repos/web/queues/main", {
    headers: { "X-CSRF-Token": await csrfToken(page) },
    data: { state: "paused" },
  });

  expect(paused.status()).toBe(403);
  expect(await paused.json()).toMatchObject({ code: "forbidden" });
});

test("a session never reaches another organization", async ({ page }) => {
  await signIn(page, /sign in as ada/i);
  await expect(page).toHaveURL(/\/orgs\/acme$/);

  const queues = await page.request.get("/api/v1/orgs/globex/queues");
  await page.goto("/orgs/globex");

  expect(queues.status()).toBe(404);
  await expect(page.getByRole("heading", { level: 1, name: "You have no access to globex" })).toBeVisible();
});

test("a refused sign-in says that it failed", async ({ page }) => {
  await signIn(page, /refuse/i);

  await expect(page).toHaveURL(/\/sign-in\?error=failed/);
  await expect(page.getByRole("alert")).toHaveText("The sign-in failed. Try again.");
});

test("a person without an organization is asked to install the App", async ({ page }) => {
  await signIn(page, /sign in as grace/i);

  await expect(page.getByRole("heading", { name: "No organization yet" })).toBeVisible();
  await expect(page.getByRole("link", { name: "sign out of GitHub" })).toBeVisible();
});
