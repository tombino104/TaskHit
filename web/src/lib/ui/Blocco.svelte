<script lang="ts">
  // L'avviso per chi è sospeso o espulso: compare solo dopo aver scritto la password giusta.
  import type { Blocco } from '../api';
  import { nomiMotivi, statoBlocco } from '../moderazione';
  let { blocco }: { blocco: Blocco } = $props();
</script>

<div class="blocco" role="alert">
  <strong>{blocco.kind === 'espulso' ? 'Il tuo account è stato espulso' : 'Il tuo account è sospeso'}</strong>
  {#if blocco.until}<span>{statoBlocco(blocco)}.</span>{/if}
  {#if blocco.tags.length}<span>Motivo: {nomiMotivi(blocco.tags)}.</span>{/if}
  {#if blocco.reason}<q>{blocco.reason}</q>{/if}
  <span class="nota">Se pensi sia un errore, parlane con un rappresentante o con l’admin.</span>
</div>

<style>
  .blocco { display: flex; flex-direction: column; gap: 6px; padding: 14px; border-left: 3px solid var(--accento); background: var(--superficie); font-size: 14px; overflow-wrap: anywhere; }
</style>
