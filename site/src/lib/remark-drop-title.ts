import { replaceSource } from "fumadocs-core/mdx-plugins/stringifier";

type File = Parameters<typeof replaceSource>[0];
type MarkdownNode = Parameters<typeof replaceSource>[1];

interface Node {
  type: string;
  depth?: number;
  children?: Node[];
}

// A page has one H1, its title from the frontmatter. A Markdown file of the
// repository that a page includes, such as INSTALL.md, keeps its own H1 for
// GitHub; `<include>` puts its content in a nested root.
export function remarkDropTitle() {
  return dropTitle;
}

function dropTitle(tree: Node, file: File) {
  const isTitle = (node: Node) => node.type === "heading" && node.depth === 1;

  for (const node of tree.children ?? []) {
    // The Markdown for agents is written from the source text, not from the tree.
    if (isTitle(node)) replaceSource(file, node as MarkdownNode, "");
    if (node.type === "root") dropTitle(node, file);
  }

  tree.children = tree.children?.filter((node) => !isTitle(node));
}
