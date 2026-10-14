import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { mockSessionCookie, problem, renderApp, sentChanges } from "@/test/app";
import { ada, answersOfMe, signedOut } from "@/test/fixtures";

const CODES = "/api/v1/device-codes";

beforeEach(() => {
  mockSessionCookie();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

function box(index: number): HTMLElement {
  return screen.getByRole("textbox", { name: `Letter ${String(index)} of 8` });
}

describe("the sign-in of the command line", () => {
  it("approves the typed code only once the person clicks, with the CSRF token", async () => {
    renderApp("/device", { ...answersOfMe(ada), [`${CODES}/WDJB-MJHT/approve`]: [204, null] });

    await userEvent.type(await screen.findByRole("textbox", { name: "Letter 1 of 8" }), "wdjb mjht");

    expect(box(8)).toHaveValue("T");
    expect(box(8)).toHaveFocus();
    expect(await sentChanges()).toEqual([]);

    await userEvent.click(screen.getByRole("button", { name: "Approve as ada" }));

    expect(await screen.findByRole("status")).toHaveTextContent("The command line acts as ada now.");
    expect(await sentChanges()).toEqual([["POST", `${CODES}/WDJB-MJHT/approve`, null, "token"]]);
  });

  it("denies a code that the person pasted, with its dash", async () => {
    renderApp("/device", { ...answersOfMe(ada), [`${CODES}/WDJB-MJHT/deny`]: [204, null] });

    await userEvent.click(await screen.findByRole("textbox", { name: "Letter 1 of 8" }));
    await userEvent.paste("wdjb-mjht");
    await userEvent.click(screen.getByRole("button", { name: "Deny" }));

    expect(await screen.findByRole("status")).toHaveTextContent("No command line signs in with this code.");
    expect(await sentChanges()).toEqual([["POST", `${CODES}/WDJB-MJHT/deny`, null, "token"]]);
  });

  it("goes back one box on a backspace in an empty box", async () => {
    renderApp("/device", answersOfMe(ada));

    await userEvent.type(await screen.findByRole("textbox", { name: "Letter 1 of 8" }), "WDJ");
    await userEvent.keyboard("{Backspace}");

    expect(box(3)).toHaveValue("");
    expect(box(3)).toHaveFocus();
  });

  it("checks the code, then says when no command line waits for it", async () => {
    renderApp("/device", {
      ...answersOfMe(ada),
      [`${CODES}/BCDF-GHJK/approve`]: [404, problem(404, "device_code_not_found")],
    });

    await userEvent.type(await screen.findByRole("textbox", { name: "Letter 1 of 8" }), "WAJBMJHT");
    await userEvent.click(screen.getByRole("button", { name: "Approve as ada" }));

    expect(screen.getByText("8 letters, as the terminal shows them.")).toBeInTheDocument();
    expect(await sentChanges()).toEqual([]);

    await userEvent.click(box(1));
    await userEvent.paste("BCDF-GHJK");
    await userEvent.click(screen.getByRole("button", { name: "Approve as ada" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("No command line waits for this code");
  });

  it("sends a person without a session to the sign-in, then back to the page", async () => {
    const router = renderApp("/device", signedOut);

    expect(await screen.findByRole("link", { name: /continue with github/i })).toHaveAttribute(
      "href",
      "/auth/github?next=%2Fdevice",
    );
    expect(router.state.location.pathname).toBe("/sign-in");
  });
});
