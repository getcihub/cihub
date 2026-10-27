import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { queueOf } from "@/test/fixtures";
import { renderInRouter } from "@/test/router";
import { QueueCards } from "./queue-cards";

describe("QueueCards", () => {
  it("shows the queues that need someone first, then in the order of the API", async () => {
    const running = queueOf("api", "main", 2);
    const paused = queueOf("web", "main");
    paused.state = "paused";
    const broken = queueOf("web", "release", 0, "unknown key `windw`");
    renderInRouter(<QueueCards org="acme" queues={[running, paused, broken]} />);

    const cards = await screen.findAllByRole("link");

    expect(cards.map((card) => card.getAttribute("href"))).toEqual([
      "/orgs/acme/repos/web/queues/main",
      "/orgs/acme/repos/web/queues/release",
      "/orgs/acme/repos/api/queues/main",
    ]);
    expect(cards[0]).toHaveTextContent("Paused");
    expect(cards[1]).toHaveTextContent("Invalid .cihub.yaml");
    expect(cards[2]).toHaveTextContent("2in the queue");
  });

  it("offers a filter from eight queues", async () => {
    const queues = Array.from({ length: 8 }, (_, index) => queueOf(`repo-${String(index)}`, "main"));
    renderInRouter(<QueueCards org="acme" queues={queues} />);

    await userEvent.type(await screen.findByRole("searchbox", { name: "Filter the queues" }), "repo-3");

    expect(screen.getAllByRole("link")).toHaveLength(1);

    await userEvent.type(screen.getByRole("searchbox"), "x");

    expect(screen.getByText("No queue matches “repo-3x”.")).toBeInTheDocument();
  });
});
