// Structured data for search engines. `<` is escaped so that no string of
// the data can close the script element.
type Json = string | number | boolean | null | undefined | Json[] | { [key: string]: Json };

export function JsonLd({ data }: { data: Record<string, Json> }) {
  return (
    <script
      type="application/ld+json"
      dangerouslySetInnerHTML={{ __html: JSON.stringify(data).replace(/</g, "\\u003c") }}
    />
  );
}
