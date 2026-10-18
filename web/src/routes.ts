import { index, rootRoute, route } from "@tanstack/virtual-file-routes";

// The addresses of the dashboard, and the route of each page in src/pages/: what it reads, where it redirects,
// and what it shows (its page, layout, loading, error and not found states).
export const routes = rootRoute("root/route.tsx", [
  index("home/route.tsx"),
  route("/sign-in", "sign-in/route.tsx"),
  route("/device", "device/route.tsx"),
  route("/welcome", "welcome/route.tsx"),
  route("/orgs", [
    index("redirects/orgs.tsx"),
    route("/$org", "org/route.tsx", [
      index("org-queues/route.tsx"),
      route("/repos", [
        index("redirects/repos.tsx"),
        route("/$repo", [
          index("repo-queues/route.tsx"),
          route("/queues", [
            index("redirects/queues.tsx"),
            route("/$queue", "queue/route.tsx", [route("/prs/$number", "pr/route.tsx")]),
          ]),
        ]),
      ]),
    ]),
  ]),
]);
