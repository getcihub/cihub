import type { Me, OrgQueue } from "../client/types.gen";

export const ada: Me = {
  login: "ada",
  orgs: [
    { slug: "acme", role: "admin" },
    { slug: "acme-labs", role: "member" },
  ],
  version: "0.2.0",
  app_install_url: "https://github.com/apps/cihub/installations/new",
};

/** A queue of `acme`, running, with `pending` PRs in line. */
export function queueOf(
  repo: string,
  name: string,
  pending = 0,
  configError: string | null = null,
): OrgQueue {
  return {
    repo,
    queue: {
      id: `queue_${repo}_${name}`,
      repo_id: `repo_${repo}`,
      name,
      target_branch: name,
      state: "running",
      mode: "single",
      state_changed_at: "2026-10-01T09:00:00Z",
      strategy: "pipelined",
      access: "ok",
      has_config_file: true,
      config_error: configError,
    },
    counts: { not_ready: 0, pending, testing: 0, pending_failure: 0, tests_passed: 0 },
    merged_last_24h: 0,
    failed_last_24h: 0,
    last_merged_at: null,
  };
}
