<script module lang="ts">
  import { marked, type Tokens } from 'marked';

  // Registered once per module load, not per component instance. Mermaid
  // fences render into a placeholder the instance script fills in after
  // mount — marked itself never touches the browser DOM.
  marked.use({
    renderer: {
      code({ text, lang }: Tokens.Code) {
        const language = (lang || '').trim().split(/\s+/)[0].toLowerCase();
        if (language === 'mermaid') {
          const encoded = encodeURIComponent(text);
          return `<div class="mermaid-block" data-mermaid="${encoded}" role="img" aria-label="diagram (rendering)"><p class="mermaid-status mono">rendering diagram…</p></div>`;
        }
        return `<figure class="code-frame"><figcaption class="code-label mono">${escapeHtml(language || 'code')}</figcaption><pre><code>${escapeHtml(text)}</code></pre></figure>`;
      },
    },
  });

  let mermaidMod: Promise<typeof import('mermaid')> | null = null;
  function loadMermaid() {
    if (!mermaidMod) {
      mermaidMod = import('mermaid').then((mod) => {
        mod.default.initialize({
          startOnLoad: false,
          securityLevel: 'strict',
          theme: 'dark',
          fontFamily: 'inherit',
        });
        return mod;
      });
    }
    return mermaidMod;
  }

  function escapeHtml(s: string): string {
    return s
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;');
  }
</script>

<script lang="ts">
  import DOMPurify from 'dompurify';
  import { tick } from 'svelte';

  let {
    markdown = '',
    locked = false,
    inline = false,
    compact = false,
  }: {
    markdown: string;
    locked?: boolean;
    inline?: boolean;
    compact?: boolean;
  } = $props();

  let container = $state<HTMLElement | undefined>(undefined);

  const html = $derived(
    DOMPurify.sanitize(
      (inline && !compact
        ? (marked.parseInline(markdown, { async: false }) as string)
        : (marked.parse(markdown, { async: false }) as string)) || '',
      { ADD_TAGS: ['foreignObject'], ADD_ATTR: ['data-mermaid'] }
    )
  );

  $effect(() => {
    // Re-run whenever `html` changes — new diagram placeholders appear
    // each time the markdown source changes (new course, new question).
    void html;
    renderDiagrams();
  });

  async function renderDiagrams() {
    await tick();
    const root = container;
    if (!root) return;
    const blocks = Array.from(
      root.querySelectorAll<HTMLElement>('.mermaid-block:not([data-rendered])')
    );
    if (blocks.length === 0) return;
    let mermaid: typeof import('mermaid').default;
    try {
      mermaid = (await loadMermaid()).default;
    } catch {
      for (const block of blocks) {
        block.dataset.rendered = '1';
        showSource(block, 'diagram renderer unavailable');
      }
      return;
    }
    for (const block of blocks) {
      block.dataset.rendered = '1';
      const source = decodeURIComponent(block.dataset.mermaid ?? '');
      const id = `mmd-${Math.random().toString(36).slice(2)}`;
      try {
        const { svg } = await mermaid.render(id, source);
        block.innerHTML = svg;
        block.setAttribute('aria-label', 'diagram');
      } catch {
        showSource(block, 'diagram could not be rendered — showing source');
      }
    }
  }

  function showSource(block: HTMLElement, note: string) {
    const source = decodeURIComponent(block.dataset.mermaid ?? '');
    block.setAttribute('aria-label', note);
    block.innerHTML = `<p class="mermaid-status mono">${escapeHtml(note)}</p><pre class="mermaid-fallback mono">${escapeHtml(source)}</pre>`;
  }

  function interceptLinks(node: HTMLElement) {
    const handler = (e: Event) => {
      const a = (e.target as HTMLElement).closest('a');
      if (a) {
        e.preventDefault();
        // Links never navigate the kiosk webview; resources open post-session.
      }
    };
    node.addEventListener('click', handler);
    return { destroy: () => node.removeEventListener('click', handler) };
  }
</script>

<div
  class="md"
  class:inline
  class:compact
  class:locked
  bind:this={container}
  use:interceptLinks
>
  {@html html}
</div>

<style>
  .md {
    user-select: text;
    font-size: 16px;
    line-height: 1.8;
  }
  .md.inline {
    display: inline;
    line-height: inherit;
    font-size: inherit;
  }
  .md.inline :global(p) {
    display: inline;
    margin: 0;
  }
  .md.compact {
    font-size: inherit;
    line-height: inherit;
    min-width: 0;
  }
  .md.compact :global(p) {
    margin: 0 0 0.65em;
  }
  .md.compact :global(p:last-child),
  .md.compact :global(pre:last-child),
  .md.compact :global(ul:last-child),
  .md.compact :global(ol:last-child) {
    margin-bottom: 0;
  }
  .md.compact :global(pre) {
    margin: 0.75em 0;
    padding: 12px 14px;
    border-radius: var(--radius-control);
    line-height: 1.55;
    max-width: 100%;
  }
  .md.compact :global(pre code) {
    display: block;
    width: max-content;
    min-width: 100%;
    font-size: 0.82em;
    white-space: pre;
  }
  .md.compact :global(ul),
  .md.compact :global(ol) {
    margin: 0.65em 0;
  }
  .md :global(h1),
  .md :global(h2),
  .md :global(h3) {
    font-family: var(--font-display);
    font-weight: 500;
    margin: 1.6em 0 0.5em;
    line-height: 1.3;
  }
  .md :global(h2) {
    font-size: 24px;
    border-bottom: 1px solid var(--border);
    padding-bottom: 8px;
  }
  .md :global(h3) {
    font-size: 19px;
  }
  .md :global(p) {
    margin: 0 0 1em;
  }
  .md :global(code) {
    font-family: var(--font-mono);
    font-size: 0.88em;
    background: var(--surface);
    padding: 2px 6px;
    border-radius: var(--radius-control);
  }
  .md :global(pre) {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    padding: 16px;
    overflow-x: auto;
  }
  .md :global(pre code) {
    background: none;
    padding: 0;
  }
  .md :global(.code-frame) {
    margin: 1em 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    background: var(--surface);
    overflow: hidden;
    max-width: 100%;
  }
  .md.compact :global(.code-frame) {
    margin: 0.75em 0;
    border-radius: var(--radius-control);
  }
  .md :global(.code-label) {
    display: block;
    padding: 6px 12px;
    border-bottom: 1px solid var(--border);
    color: var(--faint);
    font-size: 10px;
    line-height: 1.2;
    letter-spacing: 0.7px;
    text-transform: uppercase;
  }
  .md :global(.code-frame pre) {
    margin: 0;
    border: 0;
    border-radius: 0;
  }
  .md :global(a) {
    color: var(--accent);
    text-decoration: none;
    border-bottom: 1px dashed var(--accent);
    cursor: default;
  }
  .md :global(blockquote) {
    border-left: 3px solid var(--accent);
    margin: 1em 0;
    padding: 4px 0 4px 18px;
    color: var(--muted);
  }
  .md :global(ul),
  .md :global(ol) {
    padding-left: 1.4em;
  }
  .md :global(li) {
    margin-bottom: 0.4em;
  }
  .md :global(table) {
    border-collapse: collapse;
    margin: 1em 0;
  }
  .md :global(th),
  .md :global(td) {
    border: 1px solid var(--border);
    padding: 8px 12px;
    text-align: left;
  }
  .md :global(.mermaid-block) {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    margin: 1.2em 0;
    padding: 14px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    overflow-x: auto;
  }
  .md :global(.mermaid-block svg) {
    max-width: 100%;
  }
  .md :global(.mermaid-status) {
    font-size: 11px;
    color: var(--faint);
    margin: 0;
  }
  .md :global(.mermaid-fallback) {
    width: 100%;
    font-size: 12px;
    white-space: pre-wrap;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    padding: 10px;
    margin: 0;
  }
  @media (prefers-reduced-motion: reduce) {
    .md :global(.mermaid-block) {
      transition: none;
    }
  }
</style>
