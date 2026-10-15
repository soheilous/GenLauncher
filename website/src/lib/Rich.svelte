<script lang="ts">
  // Renders a translated text that uses a few markdown marks: [label](address), **bold**, *italic*, `code`.
  // Never raw HTML, so a translation can only produce these elements.
  import { getI18n } from "$lib/i18n";

  let { text }: { text: string } = $props();
  const i18n = getI18n();

  type Part = { kind: "text" | "b" | "i" | "code" | "a"; text: string; href?: string };

  const parts = $derived.by(() => {
    const out: Part[] = [];
    const re = /\*\*([^*]+)\*\*|\*([^*]+)\*|`([^`]+)`|\[([^\]]+)\]\(([^)\s]+)\)/g;
    let last = 0;
    for (const m of text.matchAll(re)) {
      if (m.index > last) out.push({ kind: "text", text: text.slice(last, m.index) });
      if (m[1] !== undefined) out.push({ kind: "b", text: m[1] });
      else if (m[2] !== undefined) out.push({ kind: "i", text: m[2] });
      else if (m[3] !== undefined) out.push({ kind: "code", text: m[3] });
      else out.push({ kind: "a", text: m[4], href: m[5] });
      last = m.index + m[0].length;
    }
    if (last < text.length) out.push({ kind: "text", text: text.slice(last) });
    return out;
  });

  // values put into the text in a right-to-left language are wrapped in invisible direction marks; an address must not carry them
  const target = (href: string) => {
    const clean = href.replace(/[⁦-⁩]/g, "");
    return clean.startsWith("/") ? i18n.href(clean) : clean;
  };
</script>

{#each parts as p}{#if p.kind === "text"}{p.text}{:else if p.kind === "b"}<strong>{p.text}</strong>{:else if p.kind === "i"}<em>{p.text}</em>{:else if p.kind === "code"}<code>{p.text}</code>{:else}<a href={target(p.href ?? "")} rel={target(p.href ?? "").startsWith("/") ? undefined : "noopener"}>{p.text}</a>{/if}{/each}
