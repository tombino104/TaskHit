<script lang="ts">
  import { onMount } from 'svelte';
  import Avatar from '../ui/Avatar.svelte';
  import Icona from '../ui/Icona.svelte';
  import { api, type Proposta, type TipoProposta } from '../api';
  import { app, avvisa, messaggio, sincronizza } from '../store.svelte';
  import { orologio } from '../orologio.svelte';
  import { colore, differenze, etichettaGruppo, gruppoDi, NOMI_RUOLO, riassunto, righeModifiche, testoAvvisi } from '../events';
  import { daSecondi, dataBreve, orario, quantoFa } from '../dates';

  /** Una decisione si può annullare per qualche secondo: parte allo scadere o quando si lascia la pagina. */
  const ATTESA = 6000;
  import { decideProposte } from '../lavori';
  const decide = $derived(!!app.utente && decideProposte(app.utente, app.meta));

  let proposte = $state<Proposta[]>([]);
  let caricate = $state(false);
  let errore = $state('');
  let scelte = $state<Record<string, boolean>>({});
  const timer = new Map<string, ReturnType<typeof setTimeout>>();

  async function carica() {
    try { proposte = (await api.proposte()).proposals; errore = ''; }
    catch (e) { errore = messaggio(e); }
    caricate = true;
  }

  async function invia(id: string) {
    clearTimeout(timer.get(id));
    if (!timer.delete(id)) return;
    try {
      await api.decidi(id, scelte[id]!);
      proposte = proposte.filter((p) => p.id !== id);
    } catch (e) {
      avvisa(messaggio(e));
    }
    delete scelte[id];
    await sincronizza().catch(() => {});
    await carica();
  }
  function scegli(id: string, approva: boolean) {
    scelte[id] = approva;
    timer.set(id, setTimeout(() => invia(id), ATTESA));
  }
  function annulla(id: string) {
    clearTimeout(timer.get(id));
    timer.delete(id);
    delete scelte[id];
  }
  const inviaTutte = () => { for (const id of [...timer.keys()]) void invia(id); };

  onMount(() => {
    void carica();
    addEventListener('pagehide', inviaTutte);
    return () => { removeEventListener('pagehide', inviaTutte); inviaTutte(); };
  });

  // Chi decide conta quelle che aspettano lui, non le proprie in attesa di altri.
  const restanti = $derived(proposte.filter((p) => (!decide || p.can_decide) && scelte[p.id] === undefined).length);
  const AZIONI: Record<TipoProposta, { testo: string; tono: string }> = {
    update: { testo: 'Propone una modifica', tono: 'pieno' },
    create: { testo: 'Propone un nuovo evento', tono: 'accento' },
    delete: { testo: 'Propone di eliminare', tono: 'tenue' },
  };
  function righe(p: Proposta): { campo: string; prima: string; dopo: string }[] {
    if (p.kind === 'delete') return [];
    if (p.kind === 'update') return p.current ? righeModifiche(app.meta, differenze(p.current, p.data)) : [];
    const d = p.data;
    return [
      { campo: 'DATA', prima: '', dopo: dataBreve(daSecondi(d.starts_at)) },
      { campo: 'ORA', prima: '', dopo: orario(d.starts_at, d.ends_at) },
      { campo: 'VISIB.', prima: '', dopo: 'Pubblico' },
      ...(d.alerts.length ? [{ campo: 'AVVISO', prima: '', dopo: testoAvvisi(d.alerts) }] : []),
      ...(d.description ? [{ campo: 'TESTO', prima: '', dopo: d.description }] : []),
    ];
  }
</script>

<div class="schermata approvazioni">
  <a href="#/inbox" class="torna"><Icona nome="sinistra" size={20} />Inbox</a>

  <header>
    <div>
      <h1>{decide ? 'Da approvare' : 'Le mie proposte'}</h1>
      {#if caricate}<span class="mono">{restanti ? `${restanti} IN ATTESA` : 'TUTTO FATTO'}</span>{/if}
    </div>
    <p class="lede">{decide ? 'Proposte in attesa. Nella classe decidono i rappresentanti, nei gruppi di lavoro il capogruppo; chi propone riceve l’esito nell’Inbox.' : 'Le tue modifiche in attesa. Riceverai l’esito nell’Inbox.'}</p>
  </header>

  {#if errore}<p class="errore" role="alert">{errore}</p>{/if}

  {#each proposte as p (p.id)}
    {@const a = AZIONI[p.kind]}
    {@const l = righe(p)}
    {@const fatta = scelte[p.id] !== undefined}
    <article class:fatta>
      <div class="autore">
        <Avatar nome={p.proposer.name} size={32} tono="neutro" />
        <span class="voce">
          <span class="nome">{p.proposer.name} <span class="ruolo">{NOMI_RUOLO[p.proposer.role].toUpperCase()}</span></span>
          <span class="tenue piccolo">{quantoFa(p.created_at, orologio.adesso)}</span>
        </span>
      </div>

      <span class="dato {a.tono}">{a.testo}</span>
      <h2 class:barrato={p.kind === 'delete'}>{p.data.title}</h2>
      <span class="riga-meta"><i class="quadratino" style:background={colore(app.meta, p.data.tag_id)}></i>{riassunto(app.meta, p.data)} · {p.kind === 'delete' ? `${dataBreve(daSecondi(p.data.starts_at))}, ${orario(p.data.starts_at, null)}` : etichettaGruppo(gruppoDi(app.meta, p.data.group_id))}</span>

      {#if l.length}
        <div role="table" aria-label="Campi" class="tabella">
          <div role="row" class="intesta">
            <span role="columnheader">CAMPO</span>
            <span role="columnheader">{p.kind === 'update' ? 'PRIMA' : ''}</span>
            <span role="columnheader">{p.kind === 'update' ? 'DOPO' : 'VALORE'}</span>
          </div>
          {#each l as r (r.campo)}
            <div role="row">
              <span role="cell" class="campo-nome">{r.campo}</span>
              <span role="cell" class="prima">{r.prima}</span>
              <span role="cell" class="dopo">{r.dopo}</span>
            </div>
          {/each}
        </div>
      {/if}

      {#if p.note}<p class="citazione">«{p.note}»</p>{/if}
      {#if p.stale}<p class="errore">L'evento è cambiato dopo questa proposta: si può solo rifiutare.</p>{/if}

      {#if !p.can_decide}
        {#if decide}<p class="nota">La tua proposta aspetta una decisione.</p>{/if}
      {:else}
        {#if fatta}
          <div class="esito" role="status">
            <span><Icona nome="spunta" size={18} spessore={1.8} />{scelte[p.id] ? 'Approvata da te' : 'Rifiutata da te'}</span>
            <button type="button" class="collegamento" onclick={() => annulla(p.id)}>Annulla</button>
          </div>
        {:else}
          <div class="bivio">
            <button type="button" class="pulsante" disabled={!app.online} onclick={() => scegli(p.id, false)}>Rifiuta</button>
            <button type="button" class="pulsante accento" disabled={!app.online || p.stale} onclick={() => scegli(p.id, true)}>Approva</button>
          </div>
        {/if}
      {/if}
    </article>
  {:else}
    {#if caricate && !errore}<p class="nota">Nessuna proposta in attesa.</p>{/if}
  {/each}
</div>

<style>
  .approvazioni { gap: 16px; }
  header { display: flex; flex-direction: column; gap: 8px; padding-bottom: 6px; }
  header div { display: flex; flex-wrap: wrap; align-items: baseline; justify-content: space-between; gap: 8px; }
  h1 { font-size: clamp(44px, 14vw, 54px); line-height: .92; }
  header .mono { font-size: 13px; color: var(--inchiostro-2); }
  .lede { font-size: 14px; }
  article { display: flex; flex-direction: column; gap: 10px; border-top: 1px solid var(--inchiostro); padding: 14px 0 6px; }
  article.fatta > :not(.esito) { opacity: .6; }
  .autore { display: flex; align-items: center; gap: 10px; }
  .nome { font-size: 14px; font-weight: 500; }
  .ruolo { margin-left: 4px; font-family: var(--font-dati); font-size: 10px; letter-spacing: .08em; font-weight: 400; padding: 1px 6px; border: 1px solid var(--filetto); color: var(--inchiostro-2); }
  .autore .piccolo { font-size: 12px; }
  .dato.accento { color: var(--accento); }
  .dato.pieno { color: var(--inchiostro); }
  h2 { margin-top: -4px; font-size: 30px; line-height: 1.02; overflow-wrap: anywhere; }
  .barrato { text-decoration: line-through; }
  .riga-meta { display: flex; align-items: center; gap: 6px; font-size: 12.5px; color: var(--inchiostro-2); }
  .tabella { display: flex; flex-direction: column; border: 1px solid var(--filetto); background: var(--superficie); font-size: 13px; }
  .tabella > div { display: grid; grid-template-columns: 64px minmax(0, 1fr) minmax(0, 1fr); gap: 8px; padding: 8px 10px; }
  .tabella > div + div { border-top: 1px solid var(--filetto); }
  .intesta { font-family: var(--font-dati); font-size: 10px; letter-spacing: .07em; color: var(--attenuato); }
  .campo-nome { font-family: var(--font-dati); font-size: 10.5px; letter-spacing: .06em; color: var(--inchiostro-2); padding-top: 1px; }
  .prima { color: var(--attenuato); text-decoration: line-through; overflow-wrap: anywhere; }
  .dopo { font-weight: 500; overflow-wrap: anywhere; }
  .citazione { padding-left: 12px; border-left: 1px solid var(--inchiostro); font-family: var(--font-titoli); font-style: italic; font-size: 18px; line-height: 1.3; color: var(--inchiostro-2); }
  .bivio { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; padding-top: 4px; }
  .esito { display: flex; align-items: center; justify-content: space-between; min-height: 46px; padding-top: 4px; font-size: 14px; }
  .esito span { display: flex; align-items: center; gap: 8px; }
</style>
