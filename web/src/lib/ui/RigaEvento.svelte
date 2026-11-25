<script lang="ts">
  import Icona from './Icona.svelte';
  import { app } from '../store.svelte';
  import { colore, etichettaGruppo, gruppoDi, materiaDi, nomeTipo, riassunto, type Evento } from '../events';
  import { ora } from '../dates';

  let { evento, compatta = false }: { evento: Evento; compatta?: boolean } = $props();
  const materia = $derived(materiaDi(app.meta, evento.tag_id));
</script>

<a href="#/evento/{evento.id}" class:compatta>
  <span class="orari">
    <span>{ora(evento.starts_at)}</span>
    {#if evento.ends_at && !compatta}<span class="tenue">{ora(evento.ends_at)}</span>{/if}
  </span>
  <span class="corpo">
    <span class="dato meta">
      <i class="quadratino" style:background={colore(app.meta, evento.tag_id)}></i>
      {#if compatta}
        <span>{riassunto(app.meta, evento)}</span>
      {:else}
        {#if materia}<span>{materia.name}</span><span class="tenue">·</span>{/if}
        <span>{nomeTipo(app.meta, evento.label_id, evento.sub_kind)}</span>
      {/if}
    </span>
    <span class="nome">{evento.title}</span>
    {#if !compatta}
      <span class="chi">
        <Icona nome={evento.privato ? 'lucchetto' : 'globo'} size={14} />
        {evento.privato ? 'Privato · Solo tu' : `Pubblico · ${etichettaGruppo(gruppoDi(app.meta, evento.group_id))}`}
        {#if evento.proposed}· modifica in attesa{/if}
      </span>
    {/if}
  </span>
</a>

<style>
  a { display: grid; grid-template-columns: 54px minmax(0, 1fr); gap: 12px; padding: 14px 0; border-bottom: 1px solid var(--filetto); }
  a.compatta { grid-template-columns: 50px minmax(0, 1fr); padding: 12px 0; }
  .orari { display: flex; flex-direction: column; font-family: var(--font-dati); font-size: 13px; padding-top: 2px; }
  .corpo { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .compatta .corpo { gap: 3px; }
  .meta { display: flex; flex-wrap: wrap; align-items: center; gap: 2px 6px; letter-spacing: .06em; }
  .nome { font-family: var(--font-titoli); font-size: 27px; line-height: 1.05; overflow-wrap: anywhere; }
  .compatta .nome { font-size: 23px; line-height: 1.1; }
  .chi { display: flex; align-items: center; gap: 5px; font-size: 12.5px; color: var(--attenuato); }
</style>
