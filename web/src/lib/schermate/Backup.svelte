<script lang="ts">
  import { t } from '../i18n';
  // Backup automatici e manuali, scaricamento e ripristino. Solo per l'admin.
  import { onMount } from 'svelte';
  import Icona from '../ui/Icona.svelte';
  import Interruttore from '../ui/Interruttore.svelte';
  import Foglio from '../ui/Foglio.svelte';
  import { api, type Backup, type ImpostazioniBackup, type StatoBackup } from '../api';
  import { app, avvisa, messaggio, ripristina } from '../store.svelte';
  import { sostituisci } from '../router.svelte';
  import { GIORNI, maiuscola, momento } from '../dates';

  type Ricorrenza = 'giorno' | 'settimana' | 'scelta';
  const TUTTI = [1, 2, 3, 4, 5, 6, 7];
  const NOMI = { auto: t('Automatico'), manuale: t('Manuale'), caricato: t('Caricato'), sicurezza: t('Prima di un ripristino') } as const;

  let stato = $state<StatoBackup | null>(null);
  let ricorrenza = $state<Ricorrenza>('giorno');
  let errore = $state('');
  let occupato = $state('');
  let daRipristinare = $state<Backup | null>(null);
  let daEliminare = $state<Backup | null>(null);
  let password = $state('');
  let erroreFoglio = $state('');

  async function carica() {
    try {
      stato = await api.backup();
      errore = '';
    } catch (e) { errore = messaggio(e); }
  }
  onMount(async () => {
    await carica();
    const n = stato?.settings.days.length;
    ricorrenza = n === 7 ? 'giorno' : n === 1 ? 'settimana' : 'scelta';
  });

  const impostazioni = $derived(stato?.settings);
  const fermo = $derived(!app.online || occupato !== '');

  async function salva(modifica: Partial<ImpostazioniBackup>) {
    if (!stato) return;
    occupato = 'salva';
    try {
      await api.salvaBackup({ ...$state.snapshot(stato.settings), ...modifica });
      await carica();
    } catch (e) { avvisa(messaggio(e)); } finally { occupato = ''; }
  }

  function scegliRicorrenza(r: Ricorrenza) {
    ricorrenza = r;
    if (!impostazioni) return;
    if (r === 'giorno') void salva({ days: TUTTI });
    else if (r === 'settimana') void salva({ days: [impostazioni.days.length === 1 ? impostazioni.days[0]! : 7] });
  }

  function toccaGiorno(g: number) {
    if (!impostazioni) return;
    if (ricorrenza === 'settimana') { void salva({ days: [g] }); return; }
    const days = impostazioni.days.includes(g) ? impostazioni.days.filter((d) => d !== g) : [...impostazioni.days, g];
    if (days.length) void salva({ days });
  }

  async function adesso() {
    occupato = 'crea';
    try {
      await api.backupAdesso();
      await carica();
      avvisa(t('Backup creato.'));
    } catch (e) { avvisa(messaggio(e)); } finally { occupato = ''; }
  }

  async function caricato(ev: Event) {
    const input = ev.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    occupato = 'carica';
    try {
      await api.caricaBackup(file);
      await carica();
      avvisa(t('Backup aggiunto all’archivio. Per usarlo premi «Ripristina».'));
    } catch (e) { avvisa(messaggio(e)); } finally { occupato = ''; }
  }

  async function conferma(ev: SubmitEvent) {
    ev.preventDefault();
    if (!daRipristinare) return;
    occupato = 'ripristina'; erroreFoglio = '';
    try {
      await ripristina(daRipristinare.name, password);
      sostituisci('accesso');
      avvisa(t('Backup ripristinato. Accedi con la password che avevi in quella data.'));
    } catch (e) { erroreFoglio = messaggio(e); } finally { occupato = ''; }
  }

  async function elimina() {
    if (!daEliminare) return;
    occupato = 'elimina';
    try {
      await api.eliminaBackup(daEliminare.name);
      daEliminare = null;
      await carica();
    } catch (e) { avvisa(messaggio(e)); } finally { occupato = ''; }
  }

  const peso = (n: number) => (n < 1_000_000 ? `${Math.max(1, Math.round(n / 1000))} kB` : `${(n / 1_000_000).toFixed(1).replace('.', ',')} MB`);
  const ultimo = $derived(stato?.items[0]);
</script>

<div class="schermata con-menu backup">
  <a href="#/profilo" class="torna"><Icona nome="sinistra" size={20} />{t('Profilo')}</a>
  <span class="dato occhio">{t('Pannello admin')}</span>
  <h1 class="titolo sottolineato">{t('Backup')}</h1>

  {#if errore}<p class="errore" role="alert">{errore}</p>{/if}

  {#if stato && impostazioni}
    <section>
      <p class="stato">{ultimo ? t('Ultimo backup: {quando}.', { quando: momento(ultimo.created_at) }) : t('Nessun backup, per ora.')}</p>
      <p class="nota">{stato.next ? t('Il prossimo automatico: {quando}.', { quando: momento(stato.next) }) : t('I backup automatici sono spenti.')}</p>
      {#if stato.error}<p class="errore" role="alert">{stato.error}</p>{/if}
      <button type="button" class="pulsante" disabled={fermo} aria-busy={occupato === 'crea'} onclick={adesso}>
        <Icona nome="aggiorna" size={18} />{occupato === 'crea' ? t('Copio i dati…') : t('Esegui un backup adesso')}
      </button>
    </section>

    <section>
      <Interruttore nome={t('Backup automatici')} descrizione={t('Se il server è spento all’orario previsto, il backup parte alla riaccensione.')}
        attivo={impostazioni.enabled} disabilitato={fermo} onchange={(enabled) => salva({ enabled })} />
    </section>

    {#if impostazioni.enabled}
      <section>
        <h2 class="dato">{t('Ricorrenza')}</h2>
        <div role="radiogroup" aria-label={t('Ricorrenza')} class="segmenti">
          <button type="button" role="radio" aria-checked={ricorrenza === 'giorno'} disabled={fermo} onclick={() => scegliRicorrenza('giorno')}>{t('Ogni giorno')}</button>
          <button type="button" role="radio" aria-checked={ricorrenza === 'settimana'} disabled={fermo} onclick={() => scegliRicorrenza('settimana')}>{t('Ogni settimana')}</button>
          <button type="button" role="radio" aria-checked={ricorrenza === 'scelta'} disabled={fermo} onclick={() => scegliRicorrenza('scelta')}>{t('Personalizzata')}</button>
        </div>
        {#if ricorrenza !== 'giorno'}
          <div class="scelte" role="group" aria-label={t('Giorni')}>
            {#each GIORNI as nome, i (nome)}
              <button type="button" class="scelta" aria-pressed={impostazioni.days.includes(i + 1)} aria-label={maiuscola(nome)} disabled={fermo} onclick={() => toccaGiorno(i + 1)}>{maiuscola(nome.slice(0, 3))}</button>
            {/each}
          </div>
        {/if}
        <label class="campo"><span class="dato">{t('Orario ({fuso})', { fuso: stato.timezone })}</span>
          <input class="mono" type="time" value={impostazioni.time} disabled={fermo} onchange={(e) => e.currentTarget.value && salva({ time: e.currentTarget.value })} /></label>
      </section>
    {/if}

    <section>
      <h2 class="dato">{t('Conserva')}</h2>
      <div class="conserva">
        <span><span class="quanti">{impostazioni.keep}</span><span>{t('backup più recenti')}</span></span>
        <span class="passo">
          <button type="button" class="tondo" aria-label={t('Conserva meno backup')} disabled={fermo || impostazioni.keep <= 1} onclick={() => salva({ keep: impostazioni.keep - 1 })}><Icona nome="meno" size={18} spessore={1.6} /></button>
          <button type="button" class="tondo" aria-label={t('Conserva più backup')} disabled={fermo || impostazioni.keep >= 90} onclick={() => salva({ keep: impostazioni.keep + 1 })}><Icona nome="piu" size={18} spessore={1.6} /></button>
        </span>
      </div>
      <p class="nota">{t('Quando ne arriva uno nuovo, quelli più vecchi oltre il limite vengono eliminati.')}</p>
    </section>

    <section>
      <h2 class="dato">{t('Archivio')}</h2>
      <p class="nota">
        {#if stato.copy.active && (stato.copy.latest_copied || !ultimo)}{t('Ogni backup viene copiato anche nella cartella scelta sul server.')}
        {:else if stato.copy.active}{t('Attenzione: l’ultimo backup non risulta nella cartella scelta sul server.')}
        {:else}{t('Questi file stanno dentro l’app: se la disinstalli o si guasta il disco, si perdono. Scaricane uno ogni tanto.')}{/if}
        {t('Ogni file contiene tutti i dati e la chiave per aprirli: conservalo al sicuro.')}
      </p>
      {#if stato.items.length}
        <ul class="elenco">
          {#each stato.items as b (b.name)}
            <li>
              <span class="voce"><span>{momento(b.created_at)}</span><span class="dato tenue">{NOMI[b.kind]} · {peso(b.size)}</span></span>
              <span class="azioni">
                <a class="collegamento" href={api.scaricaBackup(b.name)} download={b.name}>{t('Scarica')}</a>
                <button type="button" class="collegamento" disabled={fermo} onclick={() => { password = ''; erroreFoglio = ''; daRipristinare = b; }}>{t('Ripristina')}</button>
                <button type="button" class="tondo" aria-label={t('Elimina il backup del {quando}', { quando: momento(b.created_at) })} disabled={fermo} onclick={() => (daEliminare = b)}><Icona nome="cestino" size={18} /></button>
              </span>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="vuoto">{t('Nessun backup.')}</p>
      {/if}
      <label class="pulsante tratteggiato carica">
        <Icona nome="carica" size={18} />{occupato === 'carica' ? t('Controllo il file…') : t('Carica un backup salvato')}
        <input class="nascosto" type="file" accept=".tar,application/x-tar" disabled={fermo} onchange={caricato} />
      </label>
    </section>

    <p class="destinazione">{t('Cartella sul server')}<br />{stato.dir}</p>
  {:else if !errore}
    <p class="dato" role="status">{t('Caricamento…')}</p>
  {/if}
</div>

{#if daRipristinare}
  <Foglio titolo={t('Ripristina')} onclose={() => occupato || (daRipristinare = null)}>
    <form onsubmit={conferma}>
      <p>{t('Tutto torna com’era il')} <strong>{momento(daRipristinare.created_at)}</strong>{t(': eventi, persone, password e impostazioni. Ciò che è arrivato dopo sparisce.')}</p>
      <p class="nota">{t('Prima viene salvata una copia dello stato di adesso. Poi tutti dovranno accedere di nuovo, con la password che avevano in quella data.')}</p>
      <label class="campo"><span class="dato">{t('La tua password')}</span><input type="password" bind:value={password} required autocomplete="current-password" /></label>
      {#if erroreFoglio}<p class="errore" role="alert">{erroreFoglio}</p>{/if}
      <button class="pulsante accento grande" disabled={fermo} aria-busy={occupato === 'ripristina'}>{occupato === 'ripristina' ? t('Ripristino…') : t('Ripristina questo backup')}</button>
    </form>
  </Foglio>
{/if}

{#if daEliminare}
  <Foglio titolo={t('Elimina backup')} onclose={() => (daEliminare = null)}>
    <p>{t('Il backup del')} <strong>{momento(daEliminare.created_at)}</strong> {t('viene cancellato per sempre.')}</p>
    <button type="button" class="pulsante pieno grande" disabled={fermo} onclick={elimina}>{t('Elimina')}</button>
  </Foglio>
{/if}

<style>
  .backup { gap: 20px; }
  .occhio { letter-spacing: .1em; }
  .titolo { margin-top: -12px; }
  section, form { display: flex; flex-direction: column; gap: 10px; }
  .stato { font-family: var(--font-titoli); font-size: 30px; line-height: 1.08; }
  .segmenti > * { min-height: 42px; }
  .conserva { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .conserva > span:first-child { display: flex; align-items: baseline; gap: 8px; color: var(--inchiostro-2); }
  .quanti { font-family: var(--font-titoli); font-size: 56px; line-height: .9; color: var(--inchiostro); }
  .passo { display: flex; border: 1px solid var(--inchiostro); }
  .passo button { width: 46px; height: 46px; }
  .passo button + button { border-left: 1px solid var(--inchiostro); }
  .elenco > li { flex-wrap: wrap; row-gap: 0; padding: 8px 0; }
  .azioni { display: flex; align-items: center; gap: 14px; margin-left: auto; }
  .vuoto { border-top: 1px solid var(--inchiostro); padding: 16px 0; font-size: 14px; color: var(--attenuato); }
  .carica { cursor: pointer; }
  .carica:focus-within { outline: 2px solid var(--accento); outline-offset: 2px; }
  .destinazione { font-family: var(--font-dati); font-size: 11px; color: var(--attenuato); line-height: 1.6; overflow-wrap: anywhere; }
</style>
