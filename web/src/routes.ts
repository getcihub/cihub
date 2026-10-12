import { index, rootRoute, route } from "@tanstack/virtual-file-routes";

// The addresses of the dashboard, and the file of each route in src/routes/: what it reads, where it redirects,
// and the page that it shows (src/pages/) with its loading, error and not found states.
export const routes = rootRoute("root.tsx", [
  index("home.tsx"),
  route("/sign-in", "sign-in.tsx"),
  route("/orgs", [
    index("redirects/orgs.tsx"),
    route("/$org", "org.tsx", [
      index("org-queues.tsx"),
      route("/repos", [
        index("redirects/repos.tsx"),
        route("/$repo", [
          index("repo-queues.tsx"),
          route("/queues", [
            index("redirects/queues.tsx"),
            route("/$queue", "queue.tsx", [route("/prs/$number", "pr.tsx")]),
          ]),
        ]),
      ]),
    ]),
  ]),
]);
