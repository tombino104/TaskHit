<script lang="ts">
  import { t } from '../i18n';
  import Avatar from '../ui/Avatar.svelte';
  import Foglio from '../ui/Foglio.svelte';
  import Icona from '../ui/Icona.svelte';
  import Sblocco from '../ui/Sblocco.svelte';
  import { api } from '../api';
  import { app, avvisa, esci, leggiProfilo, messaggio, salvaProfilo, sincronizza, type Profilo } from '../store.svelte';
  import { NOMI_RUOLO } from '../events';
  import { riduciFoto } from '../foto';
  import { gruppiDiLavoro } from '../lavori';
  import { daChiave, mese, momento } from '../dates';

  const utente = $derived(app.utente!);
  const admin = $derived(utente.role === 'admin');
  const mieiGruppi = $derived(app.meta?.groups.filter((g) => g.mine) ?? []);
  const lavoriAperti = $derived(gruppiDiLavoro(app.meta).filter((g) => !g.work!.closed).length);
  const classe = $derived(mieiGruppi.find((g) => g.kind === 'classe')?.name);

  let profilo = $state<Profilo | null>({});
  $effect(() => {
    void app.sbloccato; void app.utente?.profile_blob;
    leggiProfilo().then((p) => (profilo = p));
  });
  const compleanno = $derived(
    profilo === null ? t('Bloccato') : profilo.compleanno ? `${daChiave(profilo.compleanno).getDate()} ${mese(daChiave(profilo.compleanno))}` : t('Non indicato'),
  );
  const nomeTema = $derived(app.preferenze.modo === 'sistema' ? t('Sistema') : app.preferenze.modo === 'chiaro' ? t('Chiaro') : t('Scuro'));
  const pluginAttivi = $derived(app.meta?.plugins.grafico.user && app.meta.plugins.grafico.instance ? 1 : 0);

  let foglio = $state<'' | 'modifica' | 'privacy'>('');
  let sblocco = $state(false);
  let nome = $state('');
  let cognome = $state('');
  let data = $state('');
  let errore = $state('');
  let occupato = $state(false);

  function apriModifica() {
    nome = utente.first_name ?? ''; cognome = utente.last_name ?? ''; data = profilo?.compleanno ?? ''; errore = ''; foglio = 'modifica';
  }
  async function salva(ev: SubmitEvent) {
    ev.preventDefault(); errore = ''; occupato = true;
    try {
      // Se il profilo cifrato non è leggibile, si cambia solo il nome e il resto non viene toccato.
      await salvaProfilo({ first_name: nome, last_name: cognome }, profilo === null || !app.sbloccato ? null : { ...profilo, compleanno: data || undefined });
      foglio = ''; avvisa(t('Profilo aggiornato.'));
    } catch (e) { errore = messaggio(e); } finally { occupato = false; }
  }
  // --- Foto ---
  let caricando = $state(false);
  async function cambiaFoto(ev: Event) {
    const input = ev.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    caricando = true;
    try { await api.salvaFoto(await riduciFoto(file)); await sincronizza(); avvisa(t('Foto aggiornata.')); }
    catch (e) { avvisa(messaggio(e)); } finally { caricando = false; }
  }
  async function togliFoto() {
    try { await api.togliFoto(); await sincronizza(); avvisa(t('Foto tolta.')); }
    catch (e) { avvisa(messaggio(e)); }
  }
  async function esciAltrove() {
    try { await api.esciAltrove(); await sincronizza(); avvisa(t('Gli altri dispositivi sono stati disconnessi.')); }
    catch (e) { avvisa(messaggio(e)); }
  }
</script>

<div class="schermata con-menu profilo">
  <div class="testa">
    <span class="dato">{t('Profilo')}</span>
    <button type="button" class="collegamento" disabled={!app.online} onclick={apriModifica}>{t('Modifica')}</button>
  </div>

  <section class="identita">
    <div class="foto">
      <Avatar nome={utente.display_name} persona={utente.id} size={108} />
      <label class="scatta" class:ferma={!app.online || caricando}>
        <Icona nome="camera" size={18} spessore={1.6} /><span class="nascosto">{t('Cambia foto profilo')}</span>
        <input class="nascosto" type="file" accept="image/*" disabled={!app.online || caricando} onchange={cambiaFoto} />
      </label>
    </div>
    <div class="chi">
      <h1>{utente.display_name}</h1>
      <div>
        <span class="ruolo">{NOMI_RUOLO[utente.role].toUpperCase()}</span>
        {#if classe}<span>{t('Classe {nome}', { nome: classe })}</span>{/if}
      </div>
    </div>
  </section>

  <dl class="scheda-dati">
    <dt>{t('Foto')}</dt><dd>{#if utente.photo}{admin ? t('Caricata') : t('La vedono i compagni')}<button type="button" class="sblocca" disabled={!app.online} onclick={togliFoto}>{t('Togli')}</button>{:else}<span class="tenue">{caricando ? t('Carico…') : t('Nessuna')}</span>{/if}</dd>
    {#if admin}
      <dt>{t('Compito')}</dt><dd>{t('Gestione del server. Non fa parte della classe.')}</dd>
    {:else}
      <dt>{t('Gruppi')}</dt><dd class:tenue={!mieiGruppi.length}>{mieiGruppi.map((g) => g.name).join(' · ') || t('Nessuno')}</dd>
      <dt>{t('Compleanno')}</dt>
      <dd class:tenue={!profilo?.compleanno}>
        {compleanno}{#if profilo === null}<button type="button" class="sblocca" onclick={() => (sblocco = true)}>{t('Sblocca')}</button>{/if}
      </dd>
    {/if}
  </dl>

  <nav aria-label={t('Impostazioni')}>
    {#snippet riga(href: string, titolo: string, valore: string)}
      <a {href}><span class="serif">{titolo}</span><span class="valore">{valore}<Icona nome="destra" size={16} /></span></a>
    {/snippet}
    {@render riga('#/aspetto', t('Aspetto e tema'), nomeTema)}
    {#if !admin}{@render riga('#/aspetto', t('Notifiche'), t('In arrivo'))}{/if}
    {#if !admin && classe}{@render riga('#/classe', t('La mia classe'), t('{n} persone', { n: mieiGruppi.find((g) => g.kind === 'classe')!.members.length }))}{/if}
    {#if !admin && (utente.role === 'rappresentante' || gruppiDiLavoro(app.meta).length)}
      {@render riga('#/lavori', t('Gruppi di lavoro'), lavoriAperti ? lavoriAperti === 1 ? t('1 aperto') : t('{n} aperti', { n: lavoriAperti }) : t('Nessuno aperto'))}
    {/if}
    {@render riga('#/plugin', t('Plugin'), pluginAttivi ? t('1 attivo') : t('Nessuno attivo'))}
    {#if admin}
      {@render riga('#/persone', t('Persone e gruppi'), app.meta?.pending ? t('{n} in attesa', { n: app.meta.pending }) : t('Admin'))}
      {@render riga('#/backup', t('Backup'), t('Admin'))}
    {:else if utente.role === 'rappresentante'}
      {@render riga('#/persone', t('Gestione classe'), app.meta?.pending ? t('{n} in attesa', { n: app.meta.pending }) : t('Inviti e gruppi'))}
    {/if}
    <button type="button" class="voce-menu" onclick={() => (foglio = 'privacy')}>
      <span class="serif">{t('Privacy e sicurezza')}</span>
      <span class="valore">{utente.sessions === 1 ? t('1 dispositivo') : t('{n} dispositivi', { n: utente.sessions })}<Icona nome="destra" size={16} /></span>
    </button>
    <button type="button" class="voce-menu esci" onclick={esci}><span class="serif">{t('Esci')}</span></button>
  </nav>

  <p class="piede">TaskHit · {location.host}<br />
    {t('Ultima sincronizzazione: {quando}', { quando: app.ultimaSync ? momento(app.ultimaSync / 1000) : t('mai') })} · {app.online ? t('dati disponibili offline') : t('sei offline')}</p>
</div>

<Sblocco bind:aperto={sblocco} riga={false} />

{#if foglio === 'modifica'}
  <Foglio titolo={t('Modifica profilo')} onclose={() => (foglio = '')}>
    <form onsubmit={salva}>
      <label class="campo"><span class="dato">{t('Nome')}</span><input bind:value={nome} required maxlength="40" autocomplete="given-name" /></label>
      <label class="campo"><span class="dato">{t('Cognome')}</span><input bind:value={cognome} required maxlength="40" autocomplete="family-name" />
        <span class="nota">{t('Se li cambi, entrerai con quelli nuovi.')}</span></label>
      {#if !admin}
        {#if app.sbloccato && profilo !== null}
          <label class="campo"><span class="dato">{t('Compleanno · cifrato')}</span><input type="date" bind:value={data} /></label>
        {:else}
          <p class="nota">{t('Il compleanno è cifrato:')} <button type="button" class="sblocca" onclick={() => { foglio = ''; sblocco = true; }}>{t('sblocca')}</button> {t('per modificarlo.')}</p>
        {/if}
      {/if}
      {#if errore}<p class="errore" role="alert">{errore}</p>{/if}
      <button class="pulsante pieno grande" disabled={occupato} aria-busy={occupato}>{t('Salva')}</button>
    </form>
  </Foglio>
{:else if foglio === 'privacy'}
  <Foglio titolo={t('Privacy e sicurezza')} onclose={() => (foglio = '')}>
    <dl class="scheda-dati">
      {#if !admin}
        <dt>{t('Privati')}</dt><dd>{t('Cifrati sul dispositivo: il server non li legge.')}</dd>
        <dt>{t('Stato')}</dt><dd>{app.sbloccato ? t('Sbloccati in questa sessione') : t('Bloccati')}{#if !app.sbloccato}<button type="button" class="sblocca" onclick={() => { foglio = ''; sblocco = true; }}>{t('Sblocca')}</button>{/if}</dd>
      {/if}
      <dt>{t('Accessi')}</dt><dd>{utente.sessions === 1 ? t('1 dispositivo collegato') : t('{n} dispositivi collegati', { n: utente.sessions })}</dd>
    </dl>
    <button type="button" class="pulsante grande" disabled={utente.sessions < 2 || !app.online} onclick={esciAltrove}>{t('Esci dagli altri dispositivi')}</button>
    <p class="nota">{t('Il cambio password arriva in una prossima versione.')}</p>
  </Foglio>
{/if}

<style>
  .testa { display: flex; align-items: center; justify-content: space-between; }
  .testa .dato { font-size: 11px; letter-spacing: .1em; }
  .identita { display: flex; align-items: flex-end; gap: 18px; padding-bottom: 18px; border-bottom: 1px solid var(--inchiostro); }
  .foto { position: relative; flex: none; display: flex; }
  .foto :global(> span) { font-family: var(--font-titoli); font-weight: 400; font-size: 46px !important; }
  .scatta { position: absolute; right: -4px; bottom: -4px; width: 44px; height: 44px; border-radius: 50%; border: 3px solid var(--sfondo); background: var(--inchiostro); color: var(--sfondo); display: flex; align-items: center; justify-content: center; cursor: pointer; }
  .scatta.ferma { opacity: .6; cursor: default; }
  .scatta:focus-within { outline: 2px solid var(--accento); outline-offset: 2px; }
  .chi { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  /* Nome e cognome su due righe, come in una testata. */
  h1 { width: min-content; max-width: 100%; font-size: 46px; line-height: .9; overflow-wrap: break-word; }
  .chi div { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; font-size: 13px; color: var(--inchiostro-2); }
  .ruolo { min-height: 24px; padding: 0 8px; display: inline-flex; align-items: center; background: var(--accento); color: var(--su-accento); font-family: var(--font-dati); font-size: 10px; letter-spacing: .08em; }
  .sblocca { margin-left: 8px; font-size: 13px; color: var(--accento); text-decoration: underline; text-underline-offset: 3px; }
  nav { display: flex; flex-direction: column; }
  nav a, .voce-menu { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 52px; border-bottom: 1px solid var(--filetto); text-align: left; }
  nav .serif { font-size: 24px; }
  .valore { display: flex; align-items: center; gap: 6px; font-size: 13px; color: var(--attenuato); }
  .esci { color: var(--attenuato); }
  .piede { font-family: var(--font-dati); font-size: 10.5px; letter-spacing: .04em; color: var(--attenuato); line-height: 1.6; overflow-wrap: anywhere; }
  form { display: flex; flex-direction: column; gap: 18px; }
</style>
