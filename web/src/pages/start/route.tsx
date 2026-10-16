import { createFileRoute } from "@tanstack/react-router";
import { z } from "zod";

import { StartPage } from "./page";

const search = z.object({
  repo: z.string().optional().catch(undefined),
  connecting: z.enum(["install", "update"]).optional().catch(undefined),
  q: z.string().optional().catch(undefined),
});

export const Route = createFileRoute("/orgs/$org/start")({
  validateSearch: search,
  component: StartPage,
});
