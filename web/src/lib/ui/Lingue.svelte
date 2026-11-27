<script lang="ts">
  import { linguaAttiva, lingue, scegliLingua, t } from '../i18n';
</script>

{#if lingue().length > 1}
  <div role="radiogroup" aria-label={t('Lingua')} class="segmenti">
    {#each lingue() as l (l.id)}
      <button type="button" role="radio" lang={l.id} aria-checked={linguaAttiva() === l.id} onclick={() => scegliLingua(l.id)}>{l.nome}</button>
    {/each}
  </div>
{/if}

<style>
  .segmenti { flex-wrap: wrap; }
  .segmenti > * { min-height: 44px; font-size: 14px; }
</style>
