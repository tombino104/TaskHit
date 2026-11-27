<script lang="ts">
  import { iniziali } from '../events';
  import { urlFoto } from '../foto';
  import { app } from '../store.svelte';

  // Con `persona` mostra la sua foto, se la conosciamo; altrimenti (o se non si carica) le iniziali.
  let { nome, persona, foto, size = 36, tono = 'tenue' }: {
    nome: string; persona?: string; foto?: number | null; size?: number; tono?: 'tenue' | 'pieno' | 'neutro';
  } = $props();

  const versione = $derived(
    foto ?? (!persona ? null : persona === app.utente?.id ? app.utente.photo
      : app.meta?.groups.flatMap((g) => g.members).find((m) => m.id === persona)?.photo),
  );
  const indirizzo = $derived(persona && versione ? urlFoto(persona, versione) : '');
  let rotto = $state('');
</script>

<span class={tono} style:width="{size}px" style:height="{size}px" style:font-size="{Math.max(12, size * 0.36)}px" aria-hidden="true">
  {#if indirizzo && rotto !== indirizzo}<img src={indirizzo} alt="" width={size} height={size} onerror={() => (rotto = indirizzo)} />
  {:else}{iniziali(nome)}{/if}
</span>

<style>
  span { flex: none; border-radius: 50%; overflow: hidden; display: inline-flex; align-items: center; justify-content: center; font-weight: 600; line-height: 1; }
  img { width: 100%; height: 100%; object-fit: cover; }
  .tenue { background: var(--accento-tenue); color: var(--accento); }
  .pieno { background: var(--inchiostro); color: var(--sfondo); }
  .neutro { background: var(--superficie); color: var(--inchiostro); border: 1px solid var(--filetto); }
</style>
