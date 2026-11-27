<script lang="ts">
  // Il rappresentante compone i gruppi di un lavoro, oppure sposta le persone tra gruppi già creati.
  import { untrack } from 'svelte';
  import Icona from '../ui/Icona.svelte';
  import Lavagna from '../ui/Lavagna.svelte';
  import { api } from '../api';
  import { app, avvisa, messaggio, sincronizza } from '../store.svelte';
  import { sostituisci, vai } from '../router.svelte';
  import { aCaso, bozzeVuote, daAssegnare, gruppiDiLavoro, MAX_GIORNI, nomeBreve, problema, type Bozza } from '../lavori';

  // «nuovo», oppure l'id di uno dei gruppi del lavoro da risistemare.
  let { quale }: { quale: string } = $props();
  const modifica = untrack(() => quale !== 'nuovo');
  const primo = untrack(() => gruppiDiLavoro(app.meta).find((g) => g.id === quale));
  const esistenti = untrack(() => (primo
    ? gruppiDiLavoro(app.meta).filter((g) => g.parent_id === primo.parent_id && g.work!.activity === primo.work!.activity && !g.work!.closed)
    : []));

  const classi = $derived(app.meta?.groups.filter((g) => g.kind === 'classe' && g.manage) ?? []);
  let sceltaClasse = $state('');
  const classeId = $derived(primo?.parent_id ?? (sceltaClasse || classi[0]?.id));
  const persone = $derived(app.meta?.groups.find((g) => g.id === classeId)?.members ?? []);

  let attivita = $state(primo?.work?.activity ?? '');
  let giorni = $state(14);
  let bozze = $state<Bozza[]>(untrack(() => (modifica
    ? esistenti.map((g) => ({ id: g.id, nome: nomeBreve(g), membri: g.members.map((m) => m.id), capo: g.work!.leader_id }))
    : bozzeVuote(2))));
  let errore = $state('');
  let occupato = $state(false);

  const DURATE: [number, string][] = [[7, '1 sett.'], [14, '2 sett.'], [30, '1 mese'], [90, '3 mesi']];
  const libere = $derived(daAssegnare(persone, bozze).length);
  const manca = $derived(
    !modifica && !attivita.trim() ? 'Scrivi il nome del lavoro.'
    : !modifica && !(Number.isInteger(giorni) && giorni >= 1 && giorni <= MAX_GIORNI) ? `La durata va da 1 a ${MAX_GIORNI} giorni.`
    : bozze.some((b) => b.membri.length && !b.capo) ? 'Scegli un capogruppo per ogni gruppo.'
    : problema(bozze),
  );

  function aggiungi() {
    let n = bozze.length + 1;
    while (bozze.some((b) => b.nome === `Gruppo ${n}`)) n++;
    bozze = [...bozze, { nome: `Gruppo ${n}`, membri: [], capo: null }];
  }

  async function salva() {
    if (manca) { errore = manca; return; }
    errore = ''; occupato = true;
    const dati = $state.snapshot(bozze);
    try {
      if (modifica) {
        for (const b of dati) await api.salvaGruppoLavoro(b.id!, b.membri, b.capo!);
      } else {
        await api.creaGruppiLavoro({ class_id: classeId!, activity: attivita.trim(), days: giorni, groups: dati.map((b) => ({ name: b.nome.trim(), member_ids: b.membri, leader_id: b.capo! })) });
      }
      await sincronizza();
      avvisa(modifica ? 'Gruppi aggiornati.' : dati.length === 1 ? 'Gruppo creato.' : `${dati.length} gruppi creati.`);
      sostituisci('lavori');
    } catch (e) {
      errore = messaggio(e);
      await sincronizza().catch(() => {});
    } finally { occupato = false; }
  }
</script>

<div class="schermata componi">
  <button type="button" class="torna" onclick={() => vai('lavori')}><Icona nome="sinistra" size={20} />Gruppi di lavoro</button>
  <h1 class="titolo sottolineato">{modifica ? attivita : 'Nuovi gruppi'}</h1>

  {#if modifica && !esistenti.length}
    <p class="lede">Non ci sono gruppi aperti da risistemare.</p>
  {:else if !classi.length}
    <p class="lede">Serve far parte di una classe come rappresentante.</p>
  {:else}
    {#if !modifica}
      <label class="campo"><span class="dato">Nome del lavoro</span><input bind:value={attivita} maxlength="60" placeholder="es. Ricerca di storia" /></label>
      {#if classi.length > 1}
        <label class="campo"><span class="dato">Classe</span>
          <select value={classeId} onchange={(e) => { sceltaClasse = e.currentTarget.value; bozze = bozze.map((b) => ({ ...b, membri: [], capo: null })); }}>
            {#each classi as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
          </select>
        </label>
      {/if}
      <fieldset>
        <legend class="dato">Durata</legend>
        <div role="radiogroup" aria-label="Durata" class="segmenti">
          {#each DURATE as [n, testo] (n)}<button type="button" role="radio" aria-checked={giorni === n} onclick={() => (giorni = n)}>{testo}</button>{/each}
        </div>
        <label class="giorni">oppure <input type="number" min="1" max={MAX_GIORNI} bind:value={giorni} /> giorni <span class="nota">(massimo {MAX_GIORNI})</span></label>
      </fieldset>
    {/if}

    <div class="strumenti">
      {#if !modifica}<button type="button" class="pulsante" onclick={aggiungi} disabled={bozze.length >= 15}><Icona nome="piu" size={16} />Gruppo</button>{/if}
      <button type="button" class="pulsante" disabled={!bozze.length} onclick={() => (bozze = aCaso(bozze, persone.map((p) => p.id)))}>A caso</button>
      <button type="button" class="pulsante" onclick={() => (bozze = bozze.map((b) => ({ ...b, membri: [], capo: null })))}>Svuota</button>
    </div>

    <Lavagna {persone} bind:bozze fisso={modifica} />

    <footer>
      {#if errore}<p class="errore" role="alert">{errore}</p>{/if}
      <p class="nota">{manca || (libere ? `${libere} ${libere === 1 ? 'persona resta' : 'persone restano'} senza gruppo.` : 'Tutto pronto.')}</p>
      <button type="button" class="pulsante pieno grande" disabled={occupato || !app.online || !!manca} aria-busy={occupato} onclick={salva}>
        {occupato ? 'Salvo…' : modifica ? 'Salva' : bozze.length === 1 ? 'Crea il gruppo' : `Crea ${bozze.length} gruppi`}
      </button>
    </footer>
  {/if}
</div>

<style>
  .componi { max-width: 900px; gap: 16px; }
  .torna { margin-bottom: -8px; }
  .titolo { font-size: clamp(40px, 11vw, 54px); overflow-wrap: anywhere; }
  legend { padding: 0 0 8px; }
  .giorni { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding-top: 10px; font-size: 14px; }
  .giorni input { width: 72px; min-height: 40px; padding: 4px 8px; border: 1px solid var(--filetto); background: var(--superficie); font-size: 16px; }
  .strumenti { display: flex; flex-wrap: wrap; gap: 8px; }
  .strumenti .pulsante { flex: 1; min-height: 42px; }
  footer { position: sticky; bottom: 0; display: flex; flex-direction: column; gap: 8px; padding: 12px 0 calc(8px + var(--basso)); background: var(--sfondo); border-top: 1px solid var(--filetto); }
</style>
