<script lang="ts">
  import { app, avvisa, leggiTutto, messaggio } from '../store.svelte';
  import { orologio } from '../orologio.svelte';
  import { colore, metaNotifica, puoPubblicare, righeModifiche } from '../events';
  import { giornoRelativo, ora } from '../dates';
  import type { Notifica } from '../api';

  type Filtro = 'tutto' | 'added' | 'modified' | 'deleted';
  let filtro = $state<Filtro>('tutto');
  const FILTRI: [Filtro, string][] = [['tutto', 'Tutto'], ['added', 'Aggiunte'], ['modified', 'Modifiche'], ['deleted', 'Eliminazioni']];
  const diritto = $derived(app.utente ? puoPubblicare(app.utente.role) : 'no');

  function azione(n: Notifica): { testo: string; tono: 'accento' | 'pieno' | 'tenue' } {
    if (n.kind === 'added') return { testo: '+ Aggiunto', tono: 'accento' };
    if (n.kind === 'modified') return { testo: '→ Modificato', tono: 'pieno' };
    if (n.kind === 'deleted') return { testo: '× Eliminato', tono: 'tenue' };
    if (n.kind === 'proposal') return { testo: '? Da approvare', tono: 'accento' };
    return n.payload.outcome === 'approved' ? { testo: '✓ Proposta approvata', tono: 'pieno' } : { testo: '× Proposta rifiutata', tono: 'tenue' };
  }
  const chi = (n: Notifica): string => {
    if (n.kind === 'proposal') return `proposta di ${n.actor ?? 'qualcuno'}`;
    if (n.kind === 'decision') return `da ${n.actor ?? 'qualcuno'}`;
    return n.payload.approver ? `proposta da ${n.actor} · approvata da ${n.payload.approver}` : `da ${n.actor ?? 'qualcuno'}`;
  };
  /** Dove porta la notifica: l'approvazione, l'evento se esiste ancora, altrimenti niente. */
  const destinazione = (n: Notifica): string | undefined =>
    n.kind === 'proposal' ? '#/approvazioni' : n.event_id && app.eventi.some((e) => e.id === n.event_id) ? `#/evento/${n.event_id}` : undefined;

  const gruppi = $derived.by(() => {
    const out: { nome: string; voci: Notifica[] }[] = [];
    for (const n of app.inbox.items) {
      if (filtro !== 'tutto' && n.kind !== filtro) continue;
      const nome = giornoRelativo(n.at, orologio.adesso);
      const g = out.at(-1);
      if (g?.nome === nome) g.voci.push(n); else out.push({ nome, voci: [n] });
    }
    return out;
  });

  async function segna() {
    try { await leggiTutto(); } catch (e) { avvisa(messaggio(e)); }
  }
</script>

<div class="schermata con-menu inbox">
  <header>
    <h1>Inbox</h1>
    {#if app.inbox.unread}<button type="button" class="collegamento" disabled={!app.online} onclick={segna}>Segna tutto come letto</button>{/if}
  </header>

  <nav aria-label="Sezioni inbox" class="schede">
    <a href="#/inbox" aria-current="page">Notifiche {#if app.inbox.unread}<span class="mono quante">{app.inbox.unread} {app.inbox.unread === 1 ? 'nuova' : 'nuove'}</span>{/if}</a>
    {#if diritto !== 'no'}
      <a href="#/approvazioni">{diritto === 'proposta' ? 'Le mie proposte' : 'Da approvare'} {#if app.inbox.to_approve}<span class="pallino">{app.inbox.to_approve}</span>{/if}</a>
    {/if}
  </nav>

  <div role="radiogroup" aria-label="Filtra per tipo di modifica" class="filtri">
    {#each FILTRI as [id, nome] (id)}
      <button type="button" role="radio" class="scelta" aria-checked={filtro === id} onclick={() => (filtro = id)}>{nome}</button>
    {/each}
  </div>

  <div>
    {#each gruppi as g (g.nome)}
      <section>
        <h2 class="dato occhiello"><span>{g.nome}</span></h2>
        {#each g.voci as n (n.id)}
          {@const a = azione(n)}
          {@const riga = n.payload.changes ? righeModifiche(app.meta, n.payload.changes)[0] : undefined}
          <a href={destinazione(n)} class="voce-inbox" class:eliminato={n.kind === 'deleted'}>
            <span class="dato testa">
              <span class={a.tono}>{a.testo}</span>
              <span class="tenue">{ora(n.at)}{#if !n.read}<i><span class="nascosto">Non letta</span></i>{/if}</span>
            </span>
            <span class="nome">{n.payload.event.title}</span>
            <span class="riga"><i class="quadratino" style:background={colore(app.meta, n.payload.event.tag_id)}></i>{metaNotifica(app.meta, n.payload.event)}</span>
            {#if riga}
              <span class="differenza">
                <span class="dato tenue">{riga.campo}</span>
                <span><s>{riga.prima}</s> → <strong>{riga.dopo}</strong></span>
              </span>
            {/if}
            <span class="chi">{chi(n)}</span>
          </a>
        {/each}
      </section>
    {:else}
      <p class="vuoto">Nessuna notifica{filtro === 'tutto' ? '' : ' di questo tipo'}. Le modifiche agli eventi pubblici compaiono qui.</p>
    {/each}
  </div>
</div>

<style>
  .inbox { gap: 14px; }
  header { display: flex; align-items: flex-end; justify-content: space-between; gap: 12px; }
  h1 { font-size: 64px; line-height: .9; }
  .quante { font-weight: 400; font-size: 12px; color: var(--inchiostro-2); }
  .filtri { display: flex; gap: 6px; overflow-x: auto; scrollbar-width: none; margin: 0 -2px; padding: 2px; }
  .filtri .scelta { flex: none; min-height: 34px; font-size: 13px; }
  section { display: flex; flex-direction: column; }
  h2 { margin-top: 8px; }
  .voce-inbox { display: flex; flex-direction: column; gap: 4px; padding: 12px 0 14px; border-bottom: 1px solid var(--filetto); }
  .testa { display: flex; align-items: center; justify-content: space-between; letter-spacing: .07em; }
  .testa .accento { color: var(--accento); }
  .testa .pieno { color: var(--inchiostro); }
  .testa .tenue { display: flex; align-items: center; gap: 8px; }
  .testa i { width: 8px; height: 8px; border-radius: 50%; background: var(--accento); }
  .nome { font-family: var(--font-titoli); font-size: 25px; line-height: 1.08; overflow-wrap: anywhere; }
  .eliminato .nome { text-decoration: line-through; color: var(--attenuato); }
  .riga { display: flex; align-items: center; gap: 6px; font-size: 12.5px; color: var(--inchiostro-2); }
  .differenza { margin-top: 4px; display: grid; grid-template-columns: 62px minmax(0, 1fr); align-items: baseline; gap: 8px; padding: 8px 10px; background: var(--superficie); border: 1px solid var(--filetto); font-family: var(--font-dati); font-size: 12px; overflow-wrap: anywhere; }
  s { color: var(--attenuato); }
  strong { font-weight: 500; }
  .chi { font-size: 12.5px; color: var(--attenuato); }
  .vuoto { padding: 18px 0; font-size: 14px; color: var(--attenuato); text-wrap: pretty; }
</style>
