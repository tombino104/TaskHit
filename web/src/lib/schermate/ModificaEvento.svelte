<script lang="ts">
  import { untrack } from 'svelte';
  import Foglio from '../ui/Foglio.svelte';
  import Icona from '../ui/Icona.svelte';
  import Sblocco from '../ui/Sblocco.svelte';
  import CampoData from '../ui/CampoData.svelte';
  import { api } from '../api';
  import { app, avvisa, messaggio, salvaEvento, sincronizza } from '../store.svelte';
  import { indietro, rotta, sostituisci } from '../router.svelte';
  import { AVVISI, commuta, puoPubblicare } from '../events';
  import { aSecondi, chiave, chiaveDi, chiaveValida, daChiave, dataBreve, ora } from '../dates';

  // Il componente viene ricreato a ogni indirizzo (vedi App.svelte): i valori iniziali si leggono una volta.
  const { modo, arg } = untrack(() => ({ modo: rotta.nome, arg: rotta.arg }));
  const origine = untrack(() => app.eventi.find((e) => e.id === (modo === 'modifica' ? arg : arg.replace(/^copia-/, ''))));
  const esistente = modo === 'modifica' ? (origine ?? null) : null;
  const ruolo = untrack(() => app.utente!.role);
  const diritto = puoPubblicare(ruolo);
  const alunno = diritto === 'no';
  const gruppi = $derived(app.meta?.groups ?? []);
  const gruppiMiei = untrack(() => app.meta?.groups.filter((g) => g.mine) ?? []);
  const tipiPermessi = untrack(() => app.meta?.labels.filter((t) => !alunno || t.personal) ?? []);
  const predefinito = tipiPermessi[0];

  let titolo = $state(origine?.title ?? '');
  let tipo = $state<string | null>(origine?.label_id ?? predefinito?.id ?? null);
  let sotto = $state<string | null>(origine?.sub_kind ?? predefinito?.subs[0] ?? null);
  let materia = $state<string | null>(origine?.tag_id ?? null);
  let giorno = $state(origine ? chiaveDi(origine.starts_at) : chiaveValida(arg) ? arg : chiave(new Date()));
  let dalle = $state(origine ? ora(origine.starts_at) : '08:00');
  let alle = $state(origine?.ends_at ? ora(origine.ends_at) : '');
  let per = $state<'classe' | 'alunno'>(origine?.subject_id ? 'alunno' : 'classe');
  let gruppo = $state(origine?.group_id ?? (gruppiMiei.find((g) => g.kind === 'classe') ?? gruppiMiei[0])?.id ?? '');
  let soggetto = $state(origine?.subject_id ?? '');
  let privato = $state(origine ? origine.privato || alunno : alunno || !gruppiMiei.length);
  let avvisi = $state([...(origine?.alerts ?? [])]);
  let descrizione = $state(origine?.description ?? '');
  let nota = $state('');
  let errore = $state('');
  let occupato = $state(false);
  let sblocco = $state(false);

  const scelto = $derived(app.meta?.labels.find((t) => t.id === tipo));
  const membri = $derived(gruppi.find((g) => g.id === gruppo)?.members ?? []);
  const proposta = $derived(!privato && diritto === 'proposta');
  const testi = $derived(
    privato ? { nota: alunno ? 'Come alunno puoi aggiungere interrogazioni ed eventi personali. Restano privati.' : 'Evento privato: lo vedi solo tu, anche offline.', cta: 'Salva evento' }
    : proposta ? { nota: `Sei un Nerd: ${esistente ? 'la modifica sarà applicata' : 'l’evento sarà pubblicato'} quando un rappresentante approva.`, cta: 'Invia per approvazione' }
    : { nota: 'L’evento sarà visibile subito al gruppo e comparirà nell’Inbox di tutti.', cta: esistente ? 'Salva modifiche' : 'Pubblica evento' },
  );
  const vietato = modo === 'modifica' && (!esistente || (!esistente.privato && alunno));

  function scegliTipo(id: string) {
    tipo = id;
    sotto = app.meta?.labels.find((t) => t.id === id)?.subs[0] ?? null;
  }

  async function salva(ev: SubmitEvent) {
    ev.preventDefault(); errore = '';
    if (!titolo.trim()) { errore = 'Scrivi un titolo.'; return; }
    if (!giorno || !dalle) { errore = 'Scegli data e ora.'; return; }
    const inizio = aSecondi(giorno, dalle);
    const fine = alle ? aSecondi(giorno, alle) : null;
    if (fine !== null && fine <= inizio) { errore = "L'ora di fine deve seguire quella di inizio."; return; }
    if (privato && !app.sbloccato) { sblocco = true; return; }
    if (!privato && per === 'alunno' && !membri.some((m) => m.id === soggetto)) { errore = 'Scegli a quale alunno si riferisce.'; return; }
    occupato = true;
    try {
      const esito = await salvaEvento(
        esistente,
        { title: titolo.trim(), description: descrizione.trim() || null, tag_id: materia, label_id: tipo, sub_kind: sotto, starts_at: inizio, ends_at: fine, alerts: $state.snapshot(avvisi) },
        privato ? { privato: true } : { privato: false, group_id: gruppo, subject_id: per === 'alunno' ? soggetto : null, nota: proposta ? nota.trim() || undefined : undefined },
      );
      avvisa(esito === 'proposed' ? 'Proposta inviata.' : esito === 'published' ? 'Evento pubblicato.' : 'Evento salvato.');
      if (modo === 'modifica') indietro('oggi'); else sostituisci(`calendario/${giorno}`);
    } catch (e) { errore = messaggio(e); } finally { occupato = false; }
  }

  // --- Nuovo tipo ---
  let nuovoTipo = $state(false);
  let nomeTipo = $state('');
  let erroreTipo = $state('');
  async function creaTipo(ev: SubmitEvent) {
    ev.preventDefault(); erroreTipo = '';
    try {
      const { id } = await api.creaTipo(nomeTipo);
      await sincronizza();
      scegliTipo(id); nomeTipo = ''; nuovoTipo = false;
    } catch (e) { erroreTipo = messaggio(e); }
  }

  // --- Nuova materia ---
  let nuovaMateria = $state(false);
  let nomeMateria = $state('');
  let coloreMateria = $state('#2A9D8F');
  let erroreMateria = $state('');
  async function creaMateria(ev: SubmitEvent) {
    ev.preventDefault(); erroreMateria = '';
    try {
      const { id } = await api.creaMateria(nomeMateria, coloreMateria);
      await sincronizza();
      materia = id; nomeMateria = ''; nuovaMateria = false;
    } catch (e) { erroreMateria = messaggio(e); }
  }
</script>

<div class="pagina">
  <header>
    <span class="dato">{modo === 'modifica' ? 'Modifica evento' : 'Nuovo evento'}</span>
    <button type="button" class="tondo" aria-label="Chiudi" onclick={() => indietro()}><Icona nome="chiudi" /></button>
  </header>

  {#if vietato}
    <div class="schermata"><p class="lede">Questo evento non è disponibile per la modifica.</p><Sblocco /></div>
  {:else}
    <form onsubmit={salva} novalidate>
      <label class="campo">
        <span class="dato">Titolo</span>
        <input class="titolo-evento" bind:value={titolo} maxlength="120" placeholder="Di cosa si tratta?" />
      </label>

      <fieldset>
        <legend class="dato">Tipo · etichetta</legend>
        <div role="radiogroup" aria-label="Tipo" class="scelte">
          {#each app.meta?.labels ?? [] as t (t.id)}
            <button type="button" role="radio" class="scelta tipo" aria-checked={tipo === t.id} disabled={alunno && !t.personal} onclick={() => scegliTipo(t.id)}>{t.name}</button>
          {/each}
          {#if diritto === 'diretto'}
            <button type="button" class="scelta nuova" onclick={() => (nuovoTipo = true)}>+ Nuovo tipo</button>
          {/if}
        </div>
        {#if scelto?.subs.length}
          <div role="radiogroup" aria-label="Modalità" class="modi">
            {#each scelto.subs as s (s)}
              <button type="button" role="radio" aria-checked={sotto === s} onclick={() => (sotto = s)}><span aria-hidden="true"><i></i></span>{s.charAt(0).toUpperCase() + s.slice(1)}</button>
            {/each}
          </div>
        {/if}
      </fieldset>

      <fieldset>
        <legend class="dato">Materia · tag</legend>
        <div role="radiogroup" aria-label="Materia" class="scelte">
          {#each app.meta?.tags ?? [] as m (m.id)}
            <button type="button" role="radio" class="scelta materia" aria-checked={materia === m.id} onclick={() => (materia = materia === m.id ? null : m.id)}>
              <i class="quadratino grande" style:background={m.color}></i>{m.name}
            </button>
          {/each}
          {#if diritto === 'diretto'}
            <button type="button" class="scelta nuova" onclick={() => (nuovaMateria = true)}>+ Nuovo tag</button>
          {/if}
        </div>
      </fieldset>

      <div class="quando">
        <CampoData etichetta="Data" tipo="date" bind:valore={giorno} testo={giorno ? dataBreve(daChiave(giorno)) : 'Scegli'} />
        <CampoData etichetta="Dalle" tipo="time" bind:valore={dalle} testo={dalle || '—'} mono />
        <CampoData etichetta="Alle" tipo="time" bind:valore={alle} testo={alle || '—'} mono />
      </div>

      <fieldset>
        <legend class="dato">Per chi</legend>
        {#if privato}
          <p class="solo-tu">{app.utente?.display_name} (tu)</p>
        {:else}
          <div class="per">
            <div role="radiogroup" aria-label="Destinatario" class="segmenti">
              <button type="button" role="radio" aria-checked={per === 'classe'} onclick={() => (per = 'classe')}>Classe</button>
              <button type="button" role="radio" aria-checked={per === 'alunno'} onclick={() => (per = 'alunno')}>Alunno</button>
            </div>
            <label class="campo">
              <span class="nascosto">Classe o gruppo</span>
              <select bind:value={gruppo} onchange={() => (soggetto = '')}>
                {#each gruppiMiei as g (g.id)}<option value={g.id}>{g.name}</option>{/each}
              </select>
            </label>
          </div>
          {#if per === 'alunno'}
            <label class="campo alunno">
              <span class="nascosto">Alunno</span>
              <select bind:value={soggetto}>
                <option value="" disabled>Scegli l'alunno</option>
                {#each membri as m (m.id)}<option value={m.id}>{m.name}{m.id === app.utente?.id ? ' (tu)' : ''}</option>{/each}
              </select>
            </label>
          {/if}
        {/if}
      </fieldset>

      <fieldset>
        <legend class="dato">Visibilità</legend>
        <div role="radiogroup" aria-label="Visibilità" class="visibilita">
          {#each [false, true] as p (p)}
            {@const motivo = esistente && esistente.privato !== p ? 'La visibilità non si cambia dopo la creazione.'
              : !p && alunno ? 'Non disponibile per gli alunni.'
              : !p && !gruppiMiei.length ? 'Serve far parte di un gruppo.' : ''}
            <button type="button" role="radio" aria-checked={privato === p} disabled={!!motivo} onclick={() => (privato = p)}>
              <Icona nome={p ? 'lucchetto' : 'globo'} size={20} />
              <span class="nome">{p ? 'Privato' : 'Pubblico'}</span>
              <span class="spiega">{motivo || (p ? 'Lo vedi solo tu. Nessuna notifica agli altri.' : 'Lo vede tutto il gruppo e finisce nell’Inbox di tutti.')}</span>
              {#if privato === p}<span class="segno" aria-hidden="true"><Icona nome="spunta" size={12} spessore={2.4} /></span>{/if}
            </button>
          {/each}
        </div>
        {#if privato && !app.sbloccato}
          <button type="button" class="collegamento" onclick={() => (sblocco = true)}>Sblocca con la password per salvare eventi privati</button>
        {/if}
      </fieldset>

      <fieldset>
        <legend class="dato">Avviso <span class="facoltativo">· facoltativo</span></legend>
        <div class="scelte">
          {#each AVVISI as a (a.id)}
            <button type="button" class="scelta" aria-pressed={avvisi.includes(a.id)} onclick={() => (avvisi = commuta(avvisi, a.id))}>{a.nome}</button>
          {/each}
        </div>
      </fieldset>

      <label class="descrizione">
        <span class="dato">Descrizione <span class="facoltativo">· facoltativa</span></span>
        <textarea rows="3" bind:value={descrizione} maxlength="4000" placeholder="Argomenti, pagine, cosa portare…"></textarea>
      </label>

      {#if proposta}
        <label class="campo"><span class="dato">Nota per chi approva <span class="facoltativo">· facoltativa</span></span><input bind:value={nota} maxlength="300" placeholder="Perché questa modifica?" /></label>
      {/if}

      <footer>
        <div>
          {#if errore}<p class="errore" role="alert">{errore}</p>{/if}
          <p class="spiegazione"><Icona nome="info" size={16} /><span>{app.online ? testi.nota : 'Sei offline: per salvare serve la connessione.'}</span></p>
          <button class="pulsante pieno grande" disabled={occupato || !app.online} aria-busy={occupato}>{occupato ? 'Salvo…' : testi.cta}</button>
        </div>
      </footer>
    </form>
  {/if}
</div>

<Sblocco bind:aperto={sblocco} riga={false} />

{#if nuovoTipo}
  <Foglio titolo="Nuovo tipo" onclose={() => (nuovoTipo = false)}>
    <form class="breve" onsubmit={creaTipo}>
      <label class="campo"><span class="dato">Nome</span><input bind:value={nomeTipo} required maxlength="40" /></label>
      {#if erroreTipo}<p class="errore" role="alert">{erroreTipo}</p>{/if}
      <button class="pulsante pieno grande">Aggiungi tipo</button>
    </form>
  </Foglio>
{/if}

{#if nuovaMateria}
  <Foglio titolo="Nuova materia" onclose={() => (nuovaMateria = false)}>
    <form class="breve" onsubmit={creaMateria}>
      <label class="campo"><span class="dato">Nome</span><input bind:value={nomeMateria} required maxlength="40" /></label>
      <label class="colore"><span class="dato">Colore</span><input type="color" bind:value={coloreMateria} /></label>
      {#if erroreMateria}<p class="errore" role="alert">{erroreMateria}</p>{/if}
      <button class="pulsante pieno grande">Aggiungi materia</button>
    </form>
  </Foglio>
{/if}

<style>
  .pagina { width: 100%; max-width: 620px; margin: 0 auto; min-height: 100dvh; display: flex; flex-direction: column; }
  header { padding: var(--alto) 16px 10px 24px; display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid var(--filetto); }
  header .dato { font-size: 11px; letter-spacing: .1em; }
  form { flex: 1; display: flex; flex-direction: column; gap: 24px; padding-top: 18px; }
  form > :not(footer) { margin-left: 24px; margin-right: 24px; }
  .titolo-evento.titolo-evento { font-family: var(--font-titoli); font-size: clamp(30px, 10vw, 38px); line-height: 1.1; padding: 4px 0 8px; }
  legend { padding: 0 0 10px; }
  .modi { display: flex; flex-wrap: wrap; gap: 18px; padding-top: 12px; }
  .modi button { display: flex; align-items: center; gap: 8px; min-height: 40px; font-size: 14px; }
  .modi span { width: 18px; height: 18px; border-radius: 50%; border: 1.5px solid var(--inchiostro); display: flex; align-items: center; justify-content: center; }
  .modi i { width: 8px; height: 8px; border-radius: 50%; }
  .modi [aria-checked='true'] i { background: var(--inchiostro); }
  .quando { display: grid; grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr) minmax(0, 1fr); gap: 10px; }
  .per { display: flex; align-items: stretch; gap: 10px; }
  .per .segmenti > * { flex: none; padding: 0 14px; }
  .per .campo { flex: 1; min-width: 0; }
  .alunno { margin-top: 10px; }
  .solo-tu { padding: 8px 0; border-bottom: 1px solid var(--filetto); }
  .visibilita { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
  .visibilita button { position: relative; display: flex; flex-direction: column; align-items: flex-start; gap: 6px; padding: 14px 14px 12px; text-align: left; border: 1px solid var(--filetto); min-height: 112px; }
  .visibilita [aria-checked='true'] { border: 1.5px solid var(--inchiostro); background: var(--superficie); }
  .nome { font-family: var(--font-titoli); font-size: 24px; line-height: 1; }
  .spiega { font-size: 12.5px; line-height: 1.35; color: var(--inchiostro-2); }
  .segno { position: absolute; top: 10px; right: 10px; width: 20px; height: 20px; border-radius: 50%; background: var(--inchiostro); color: var(--sfondo); display: flex; align-items: center; justify-content: center; }
  .facoltativo { color: var(--attenuato); text-transform: none; letter-spacing: 0; }
  .descrizione { display: flex; flex-direction: column; gap: 8px; }
  footer { position: sticky; bottom: 0; z-index: 5; margin-top: auto; background: var(--sfondo); border-top: 1px solid var(--filetto); }
  footer div { display: flex; flex-direction: column; gap: 10px; padding: 14px 24px calc(16px + var(--basso)); }
  .spiegazione { display: flex; gap: 8px; align-items: flex-start; font-size: 12.5px; line-height: 1.4; color: var(--inchiostro-2); }
  .spiegazione :global(svg) { margin-top: 1px; }
  form.breve { padding: 0; gap: 18px; }
  form.breve > * { margin: 0; }
  .colore { display: flex; align-items: center; justify-content: space-between; }
  .colore input { width: 64px; height: 44px; padding: 2px; border: 1px solid var(--filetto); background: var(--superficie); }
</style>
