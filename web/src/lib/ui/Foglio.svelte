<script lang="ts">
  // Pannello che sale dal basso (centrato su schermi larghi). Usa <dialog>: fuoco e tasto Esc li gestisce il browser.
  import type { Snippet } from 'svelte';
  let { titolo, onclose, azione, children }: { titolo: string; onclose: () => void; azione?: Snippet; children: Snippet } = $props();
  const id = $props.id();

  function apri(d: HTMLDialogElement) {
    d.showModal();
    return () => d.close();
  }
</script>

<dialog {@attach apri} aria-labelledby={id}
  oncancel={(e) => { e.preventDefault(); onclose(); }}
  onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}>
  <div class="foglio">
    <span class="maniglia" aria-hidden="true"></span>
    <div class="testa">
      <h2 {id}>{titolo}</h2>
      {#if azione}{@render azione()}{:else}<button type="button" class="collegamento" onclick={onclose}>Chiudi</button>{/if}
    </div>
    {@render children()}
  </div>
</dialog>

<style>
  dialog { position: fixed; inset: auto 0 0 0; width: 100%; max-width: 620px; max-height: calc(100dvh - 48px); margin: 0 auto; padding: 0; border: 0; border-radius: 18px 18px 0 0; background: var(--sfondo); color: var(--inchiostro); overflow-y: auto; overscroll-behavior: contain; }
  dialog::backdrop { background: rgba(15, 15, 18, .5); }
  .foglio { display: flex; flex-direction: column; gap: 20px; padding: 10px var(--margine) calc(28px + var(--basso)); }
  .maniglia { align-self: center; width: 36px; height: 4px; border-radius: 2px; background: var(--filetto); }
  .testa { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; border-bottom: 1px solid var(--inchiostro); padding-bottom: 8px; }
  h2 { font-size: 36px; line-height: 1.05; }
  @media (min-width: 900px) {
    dialog { inset: 0; margin: auto; border-radius: 14px; width: min(480px, calc(100% - 48px)); height: fit-content; }
    .maniglia { display: none; }
    .foglio { padding-top: 20px; }
  }
</style>
