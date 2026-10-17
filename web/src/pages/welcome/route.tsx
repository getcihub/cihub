import { createFileRoute } from "@tanstack/react-router";
import { z } from "zod";

import { WelcomePage } from "./page";

const search = z.object({
  request: z.literal("sent").optional().catch(undefined),
});

export const Route = createFileRoute("/welcome")({
  validateSearch: search,
  component: WelcomePage,
});
