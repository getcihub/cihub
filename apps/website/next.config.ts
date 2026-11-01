import { createMDX } from "fumadocs-mdx/next";
import type { NextConfig } from "next";
import { join } from "node:path";

const withMDX = createMDX();

// Anyone may read these files, from a browser tool too.
const machineFileHeaders = (contentType: string) => [
  { key: "Content-Type", value: contentType },
  { key: "Access-Control-Allow-Origin", value: "*" },
];

const config: NextConfig = {
  reactStrictMode: true,
  // The root of the repository: a page includes INSTALL.md, outside apps/website/.
  turbopack: { root: join(import.meta.dirname, "..", "..") },
  // @cihub/icons is TypeScript source, compiled by each app.
  transpilePackages: ["@cihub/icons"],
  headers() {
    return Promise.resolve([
      { source: "/schema/:path*", headers: machineFileHeaders("application/schema+json") },
      { source: "/openapi/:path*", headers: machineFileHeaders("application/json") },
    ]);
  },
  // PostHog receives its events on this domain (src/instrumentation-client.ts);
  // its API ends some paths with a slash, which Next.js would redirect.
  rewrites() {
    return Promise.resolve([
      { source: "/ingest/static/:path*", destination: "https://eu-assets.i.posthog.com/static/:path*" },
      { source: "/ingest/array/:path*", destination: "https://eu-assets.i.posthog.com/array/:path*" },
      { source: "/ingest/:path*", destination: "https://eu.i.posthog.com/:path*" },
    ]);
  },
  skipTrailingSlashRedirect: true,
  // The paths that an agent guesses for the installation.
  redirects() {
    return Promise.resolve(
      ["/install", "/quickstart", "/getting-started", "/docs/quickstart", "/docs/getting-started"].map(
        (source) => ({ source, destination: "/docs/install", permanent: true }),
      ),
    );
  },
};

export default withMDX(config);
