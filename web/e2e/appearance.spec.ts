// The theme "System" follows the setting of the system while the page is open;
// a theme that the person chose does not.
import { expect, test } from "@playwright/test";

test("the theme follows the system until the person chooses one", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/sign-in");
  const root = page.locator("html");

  await expect(root).toHaveAttribute("data-theme", "light");

  await page.emulateMedia({ colorScheme: "dark" });

  await expect(root).toHaveAttribute("data-theme", "dark");

  await page.evaluate(() => {
    localStorage.setItem("theme", "dark");
  });
  await page.emulateMedia({ colorScheme: "light" });

  await expect(root).toHaveAttribute("data-theme", "dark");
});
