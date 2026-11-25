<script lang="ts">
  import Avatar from '../ui/Avatar.svelte';
  import Foglio from '../ui/Foglio.svelte';
  import Icona from '../ui/Icona.svelte';
  import Sblocco from '../ui/Sblocco.svelte';
  import { api, type VoceStoria } from '../api';
  import { app, avvisa, eliminaEvento, messaggio } from '../store.svelte';
  import { indietro, rotta, vai } from '../router.svelte';
  import {
    etichettaGruppo, fraseStoria, gruppoDi, materiaDi, NOMI_RUOLO, puoPubblicare, testoAvvisi, testoSu, tipoDi,
  } from '../events';
  import { daSecondi, dataLunga, maiuscola, meseBreve, momento, nomeGiorno, ora, orario } from '../dates';

  const evento = $derived(app.eventi.find((e) => e.id === rotta.arg));
  const materia = $derived(evento ? materiaDi(app.meta, evento.tag_id) : undefined);
  const tinta = $derived(materia?.color ?? '#B4B3BA');
  const inizio = $derived(evento ? daSecondi(evento.starts_at) : new Date());
  const gruppo = $derived(gruppoDi(app.meta, evento?.group_id ?? null));
  const alunno = $derived(gruppo?.members.find((m) => m.id === evento?.subject_id)?.name);
  const diritto = $derived(!evento || !app.utente ? 'no' : evento.privato ? 'diretto' : puoPubblicare(app.utente.role));
  const modificato = $derived(!!evento && evento.rev > 1);

  let storia = $state<VoceStoria[]>([]);
  // Si ricarica quando cambia l'evento o la sua revisione.
  $effect(() => {
    const id = evento?.id, privato = evento?.privato;
    void evento?.rev;
    storia = [];
    if (!id || privato) return;
    let valida = true;
    api.storia(id).then((r) => { if (valida) storia = r.history; }).catch(() => { /* offline */ });
    return () => { valida = false; };
  });
  /** Per i privati il server non ha cronologia: bastano creazione e ultima modifica. */
  const cronologia = $derived.by(() => {
    if (!evento) return [];
    if (!evento.privato) return storia.map((v) => ({ at: v.at, testo: fraseStoria(app.meta, v) }));
    const voci = [{ at: evento.created_at, testo: "Hai creato l'evento." }];
    if (modificato) voci.unshift({ at: evento.updated_at, testo: "Hai modificato l'evento." });
    return voci;
  });

  let foglio = $state<'' | 'azioni' | 'elimina' | 'avvisi'>('');
  let nota = $state('');
  let errore = $state('');
  let occupato = $state(false);

  async function elimina() {
    if (!evento) return;
    errore = ''; occupato = true;
    try {
      const esito = await eliminaEvento(evento, nota.trim() || undefined);
      foglio = '';
      avvisa(esito === 'proposed' ? 'Proposta di eliminazione inviata.' : 'Evento eliminato.');
      indietro('oggi');
    } catch (e) { errore = messaggio(e); } finally { occupato = false; }
  }
</script>

{#if !evento}
  <div class="schermata">
    <button type="button" class="torna" onclick={() => indietro()}><Icona nome="sinistra" size={20} />Indietro</button>
    <h1 class="titolo">Evento non disponibile</h1>
    <p class="lede">Forse è stato eliminato, oppure è un evento privato ancora bloccato.</p>
    <Sblocco />
  </div>
{:else}
  <article class="dettaglio">
    <header style:background={tinta} style:color={testoSu(tinta)}>
      <div class="interno">
        <div class="comandi">
          <button type="button" class="cerchio" aria-label="Indietro" onclick={() => indietro()}><Icona nome="sinistra" size={20} spessore={1.6} /></button>
          <button type="button" class="cerchio" aria-label="Altre azioni" onclick={() => (foglio = 'azioni')}><Icona nome="altro" size={20} /></button>
        </div>
        <div class="quando">
          <div class="giorno">
            <span class="mono">{nomeGiorno(inizio).toUpperCase()}</span>
            <span class="numero">{inizio.getDate()}</span>
          </div>
          <div class="mono ore">
            <span>{meseBreve(inizio).toUpperCase()} {inizio.getFullYear()}</span>
            <span class="ora">{ora(evento.starts_at)}</span>
            {#if evento.ends_at}<span class="ora">{ora(evento.ends_at)}</span>{/if}
          </div>
        </div>
      </div>
    </header>

    <div class="corpo">
      <div class="etichette">
        <span class="dato tipo">{[tipoDi(app.meta, evento.label_id)?.name ?? 'Evento', evento.sub_kind].filter(Boolean).join(' · ')}</span>
        {#if materia}<span class="dato materia"><i class="quadratino" style:background={materia.color}></i>{materia.name}</span>{/if}
      </div>

      <h1>{evento.title}</h1>

      {#if evento.proposed}
        <p class="errore">Hai proposto una modifica: aspetta l'approvazione di un rappresentante o dell'admin.</p>
      {/if}

      <div class="autore">
        <Avatar nome={evento.created_by.name} size={34} />
        <div>
          <span><strong>{evento.created_by.name}</strong> · {NOMI_RUOLO[evento.created_by.role]} · creato il {momento(evento.created_at).split(',')[0]}</span>
          {#if modificato}
            <span class="tenue">Modificato da {evento.updated_by.name} ({NOMI_RUOLO[evento.updated_by.role]}) il {momento(evento.updated_at)}{evento.approved_by ? ' · approvato' : ''}</span>
          {/if}
        </div>
      </div>

      <dl class="scheda-dati">
        <dt>Quando</dt>
        <dd>{dataLunga(inizio)}, {orario(evento.starts_at, evento.ends_at)}</dd>
        <dt>Per</dt>
        <dd>{evento.privato ? 'Solo tu' : alunno ? `${alunno} · ${etichettaGruppo(gruppo)}` : etichettaGruppo(gruppo)}</dd>
        <dt>Visibilità</dt>
        <dd>
          <Icona nome={evento.privato ? 'lucchetto' : 'globo'} size={16} />
          {evento.privato ? 'Privato, lo vedi solo tu' : gruppo?.kind === 'gruppo' ? 'Pubblico, lo vede tutto il gruppo' : 'Pubblico, lo vede tutta la classe'}
        </dd>
        <dt>Avviso</dt>
        <dd>{testoAvvisi(evento.alerts)}</dd>
      </dl>

      {#if evento.description}
        <section class="argomento">
          <h2 class="dato">Argomento</h2>
          <p>{evento.description}</p>
        </section>
      {/if}

      {#if cronologia.length}
        <section class="cronologia">
          <h2 class="dato">Cronologia</h2>
          <ol>
            {#each cronologia as v, i (i)}
              <li>
                <i aria-hidden="true" class:ultima={i === 0}></i>
                <span class="mono tenue">{momento(v.at).replace(',', ' ·').toUpperCase()}</span>
                <span>{v.testo}</span>
              </li>
            {/each}
          </ol>
        </section>
      {/if}
    </div>
  </article>

  {#if diritto !== 'no'}
    <div role="toolbar" aria-label="Azioni evento" class="strumenti">
      <a href="#/modifica/{evento.id}" class="modifica"><Icona nome="matita" size={18} spessore={1.6} />{diritto === 'proposta' ? 'Proponi modifica' : 'Modifica'}</a>
      <span aria-hidden="true"></span>
      <button type="button" aria-label="Promemoria" onclick={() => (foglio = 'avvisi')}><Icona nome="campana" size={19} spessore={1.6} /></button>
      <span aria-hidden="true"></span>
      <button type="button" aria-label={diritto === 'proposta' ? "Proponi di eliminare l'evento" : 'Elimina evento'} onclick={() => (foglio = 'elimina')}><Icona nome="cestino" size={19} spessore={1.6} /></button>
    </div>
  {/if}

  {#if foglio === 'azioni'}
    <Foglio titolo="Azioni" onclose={() => (foglio = '')}>
      <button type="button" class="pulsante grande" onclick={() => vai(`nuovo/copia-${evento.id}`)}>Duplica evento</button>
      {#if diritto !== 'no'}
        <button type="button" class="pulsante grande" onclick={() => (foglio = 'elimina')}>{diritto === 'proposta' ? 'Proponi di eliminarlo' : 'Elimina evento'}</button>
      {/if}
    </Foglio>
  {:else if foglio === 'avvisi'}
    <Foglio titolo="Promemoria" onclose={() => (foglio = '')}>
      <p class="lede">Avvisi scelti: <strong>{testoAvvisi(evento.alerts)}</strong>.</p>
      <p class="nota">Le notifiche sul dispositivo arrivano in una prossima versione: per ora l'avviso viene solo salvato con l'evento.</p>
      <a href="#/modifica/{evento.id}" class="pulsante pieno grande">Cambia avvisi</a>
    </Foglio>
  {:else if foglio === 'elimina'}
    <Foglio titolo={diritto === 'proposta' ? 'Proponi di eliminare' : 'Eliminare?'} onclose={() => { foglio = ''; errore = ''; }}>
      <p class="lede">«{evento.title}», {dataLunga(inizio).toLowerCase()}.
        {diritto === 'proposta' ? 'L’evento resta finché un rappresentante o l’admin approva.' : evento.privato ? 'Il contenuto cifrato viene cancellato e non si può recuperare.' : 'Chi fa parte del gruppo riceve una notifica. Non si può annullare.'}</p>
      {#if diritto === 'proposta'}
        <label class="campo"><span class="dato">Nota per chi approva · facoltativa</span><input bind:value={nota} maxlength="300" placeholder={maiuscola('perché va eliminato?')} /></label>
      {/if}
      {#if errore}<p class="errore" role="alert">{errore}</p>{/if}
      <button type="button" class="pulsante pieno grande" disabled={occupato || !app.online} aria-busy={occupato} onclick={elimina}>
        {diritto === 'proposta' ? 'Invia per approvazione' : 'Elimina evento'}
      </button>
    </Foglio>
  {/if}
{/if}

<style>
  .dettaglio { line-height: 1.5; padding-bottom: calc(120px + var(--basso)); }
  header { min-height: 318px; display: flex; }
  .interno { width: 100%; max-width: 620px; margin: 0 auto; padding: var(--alto) 16px 20px 24px; display: flex; flex-direction: column; justify-content: space-between; gap: 24px; }
  .comandi { display: flex; justify-content: space-between; margin-left: -8px; }
  .cerchio { width: 44px; height: 44px; border: 1px solid currentColor; border-radius: 50%; display: flex; align-items: center; justify-content: center; }
  header :focus-visible { outline-color: currentColor; }
  .quando { display: flex; align-items: flex-end; justify-content: space-between; gap: 12px; }
  .giorno { display: flex; flex-direction: column; }
  .giorno .mono { font-size: 12px; letter-spacing: .1em; }
  .numero { font-family: var(--font-titoli); font-size: clamp(140px, 45vw, 176px); line-height: .78; letter-spacing: -.03em; }
  .ore { display: flex; flex-direction: column; align-items: flex-end; gap: 2px; font-size: 12px; letter-spacing: .06em; padding: 0 8px 4px 0; }
  .ora { font-size: 18px; letter-spacing: 0; }
  .corpo { max-width: 620px; margin: 0 auto; padding: 22px 24px 0; display: flex; flex-direction: column; gap: 18px; }
  .etichette { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  .tipo { min-height: 26px; padding: 0 10px; border: 1px solid var(--inchiostro); display: inline-flex; align-items: center; color: var(--inchiostro); }
  .materia { display: inline-flex; align-items: center; gap: 6px; }
  .materia .quadratino { width: 9px; height: 9px; }
  h1 { font-size: clamp(40px, 13vw, 50px); line-height: .98; letter-spacing: -.01em; text-wrap: balance; overflow-wrap: anywhere; }
  .autore { display: flex; gap: 12px; align-items: flex-start; padding-bottom: 16px; border-bottom: 1px solid var(--filetto); }
  .autore div { display: flex; flex-direction: column; gap: 2px; font-size: 13px; color: var(--inchiostro-2); }
  .autore strong { color: var(--inchiostro); font-weight: 600; }
  .scheda-dati { grid-template-columns: 96px minmax(0, 1fr); }
  .argomento { display: flex; flex-direction: column; gap: 8px; padding-top: 4px; }
  .argomento p { font-size: 16px; line-height: 1.6; text-wrap: pretty; white-space: pre-line; overflow-wrap: anywhere; display: flow-root; }
  .argomento p::first-letter { float: left; font-family: var(--font-titoli); font-size: 66px; line-height: .8; padding: 6px 8px 0 0; color: var(--accento); }
  .cronologia { padding-top: 8px; }
  .cronologia h2 { margin-bottom: 6px; }
  ol { margin-left: 4px; border-left: 1px solid var(--inchiostro); }
  li { position: relative; display: flex; flex-direction: column; gap: 1px; padding: 6px 0 10px 16px; font-size: 14px; }
  li .mono { font-size: 11px; }
  li i { position: absolute; left: -5px; top: 11px; width: 9px; height: 9px; border-radius: 50%; background: var(--sfondo); border: 1px solid var(--inchiostro); }
  li i.ultima { background: var(--accento); border-color: var(--accento); }
  .strumenti { position: fixed; z-index: 10; left: 50%; bottom: calc(24px + var(--basso)); transform: translateX(-50%); display: flex; align-items: center; background: var(--inchiostro); color: var(--sfondo); border-radius: 30px; padding: 4px; box-shadow: 0 10px 30px rgba(0, 0, 0, .18); white-space: nowrap; }
  .strumenti a, .strumenti button { color: var(--sfondo); height: 48px; display: flex; align-items: center; justify-content: center; border-radius: 24px; }
  .modifica { padding: 0 18px; gap: 8px; font-size: 14px; font-weight: 500; }
  .strumenti button { width: 52px; }
  .strumenti span { width: 1px; height: 22px; background: rgba(127, 127, 127, .45); }
  @media (min-width: 900px) { header .interno { padding-top: 28px; } .strumenti { left: calc(50% + 135px); } }
</style>
