import { useState, type ReactNode } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Icon } from "../../Icon";

const text = (n: ReactNode): string =>
  typeof n === "string" || typeof n === "number" ? String(n)
  : Array.isArray(n) ? n.map(text).join("")
  : n && typeof n === "object" && "props" in n ? text((n.props as { children?: ReactNode }).children)
  : "";

function CodeBlock({ children }: { children?: ReactNode }) {
  const [copied, setCopied] = useState(false);
  const copy = () => navigator.clipboard.writeText(text(children).replace(/\n$/, ""))
    .then(() => { setCopied(true); setTimeout(() => setCopied(false), 1200); }).catch(() => {});
  return (
    <div className="group relative">
      <pre className="overflow-x-auto rounded-md border border-line-soft bg-chrome px-3 py-2 font-mono text-xs leading-[1.6]">{children}</pre>
      <button type="button" className="btn ghost sm absolute top-1 right-1 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
        aria-label="Copy code" title={copied ? "Copied" : "Copy"} onClick={copy}><Icon name={copied ? "check" : "copy"} /></button>
    </div>
  );
}

const C: Components = {
  pre: ({ children }) => <CodeBlock>{children}</CodeBlock>,
  code: ({ className, children }) => className
    ? <code className={className}>{children}</code>
    : <code className="rounded-sm bg-chrome px-1 py-px font-mono text-[0.92em]">{children}</code>,
  a: ({ href, children }) => (
    <a href={href} title={href} className="text-text underline decoration-accent underline-offset-2 hover:decoration-text"
      onClick={(e) => { e.preventDefault(); if (href && /^(https?|mailto):/i.test(href)) openUrl(href).catch(() => {}); }}>{children}</a>
  ),
  h1: ({ children }) => <h3 className="text-[15px] font-semibold">{children}</h3>,
  h2: ({ children }) => <h3 className="text-sm font-semibold">{children}</h3>,
  h3: ({ children }) => <h4 className="font-semibold">{children}</h4>,
  ul: ({ children }) => <ul className="list-disc space-y-0.5 pl-5">{children}</ul>,
  ol: ({ children }) => <ol className="list-decimal space-y-0.5 pl-5">{children}</ol>,
  blockquote: ({ children }) => <blockquote className="border-l border-line pl-3 text-text-2">{children}</blockquote>,
  table: ({ children }) => <div className="overflow-x-auto"><table className="border-collapse text-xs">{children}</table></div>,
  th: ({ children }) => <th className="border border-line-soft bg-surface px-2 py-1 text-left font-semibold">{children}</th>,
  td: ({ children }) => <td className="border border-line-soft px-2 py-1 align-top">{children}</td>,
  hr: () => <hr className="border-line-soft" />,
  img: ({ alt }) => <span className="text-text-2">[image{alt ? `: ${alt}` : ""}]</span>,
};

/** Model text as GitHub-flavoured Markdown. Raw HTML is not rendered. */
export function Markdown({ children, className = "" }: { children: string; className?: string }) {
  return (
    <div className={`flex flex-col gap-2 ${className}`}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={C}>{children}</ReactMarkdown>
    </div>
  );
}
