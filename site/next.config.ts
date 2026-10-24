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
  // The root of the repository: a page includes INSTALL.md, outside site/.
  turbopack: { root: join(import.meta.dirname, "..") },
  headers() {
    return Promise.resolve([
      { source: "/schema/:path*", headers: machineFileHeaders("application/schema+json") },
      { source: "/openapi/:path*", headers: machineFileHeaders("application/json") },
    ]);
  },
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
