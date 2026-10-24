import { RootProvider } from "fumadocs-ui/provider/next";
import type { Metadata } from "next";

import { siteDescription, siteName, siteUrl } from "@/lib/shared";

import "./global.css";

export const metadata: Metadata = {
  metadataBase: new URL(siteUrl),
  title: { default: "CIHub: the self-hosted merge queue for GitHub", template: `%s | ${siteName}` },
  description: siteDescription,
  applicationName: siteName,
  openGraph: { siteName, type: "website", locale: "en_US" },
  twitter: { card: "summary_large_image" },
};

export default function Layout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className="flex min-h-screen flex-col">
        <RootProvider>{children}</RootProvider>
      </body>
    </html>
  );
}
