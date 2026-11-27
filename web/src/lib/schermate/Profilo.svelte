<script lang="ts">
  import Avatar from '../ui/Avatar.svelte';
  import Foglio from '../ui/Foglio.svelte';
  import Icona from '../ui/Icona.svelte';
  import Sblocco from '../ui/Sblocco.svelte';
  import { api } from '../api';
  import { app, avvisa, esci, leggiProfilo, messaggio, salvaProfilo, sincronizza, type Profilo } from '../store.svelte';
  import { NOMI_RUOLO } from '../events';
  import { daChiave, mese, momento } from '../dates';

  const utente = $derived(app.utente!);
  const admin = $derived(utente.role === 'admin');
  const mieiGruppi = $derived(app.meta?.groups.filter((g) => g.mine) ?? []);
  const classe = $derived(mieiGruppi.find((g) => g.kind === 'classe')?.name);

  let profilo = $state<Profilo | null>({});
  $effect(() => {
    void app.sbloccato; void app.utente?.profile_blob;
    leggiProfilo().then((p) => (profilo = p));
  });
  const compleanno = $derived(
    profilo === null ? 'Bloccato' : profilo.compleanno ? `${daChiave(profilo.compleanno).getDate()} ${mese(daChiave(profilo.compleanno))}` : 'Non indicato',
  );
  const nomeTema = $derived(app.preferenze.modo === 'sistema' ? 'Sistema' : app.preferenze.modo === 'chiaro' ? 'Chiaro' : 'Scuro');
  const pluginAttivi = $derived(app.meta?.plugins.grafico.user && app.meta.plugins.grafico.instance ? 1 : 0);

  let foglio = $state<'' | 'modifica' | 'privacy'>('');
  let sblocco = $state(false);
  let nome = $state('');
  let data = $state('');
  let errore = $state('');
  let occupato = $state(false);

  function apriModifica() {
    nome = utente.display_name; data = profilo?.compleanno ?? ''; errore = ''; foglio = 'modifica';
  }
  async function salva(ev: SubmitEvent) {
    ev.preventDefault(); errore = ''; occupato = true;
    try {
      // Se il profilo cifrato non è leggibile, si cambia solo il nome e il resto non viene toccato.
      await salvaProfilo(nome, profilo === null || !app.sbloccato ? null : { ...profilo, compleanno: data || undefined });
      foglio = ''; avvisa('Profilo aggiornato.');
    } catch (e) { errore = messaggio(e); } finally { occupato = false; }
  }
  async function esciAltrove() {
    try { await api.esciAltrove(); await sincronizza(); avvisa('Gli altri dispositivi sono stati disconnessi.'); }
    catch (e) { avvisa(messaggio(e)); }
  }
</script>

<div class="schermata con-menu profilo">
  <div class="testa">
    <span class="dato">Profilo</span>
    <button type="button" class="collegamento" disabled={!app.online} onclick={apriModifica}>Modifica</button>
  </div>

  <section class="identita">
    <div class="foto">
      <Avatar nome={utente.display_name} size={108} />
      <button type="button" aria-label="Cambia foto profilo (in arrivo)" title="Foto profilo: in arrivo" disabled><Icona nome="camera" size={18} spessore={1.6} /></button>
    </div>
    <div class="chi">
      <h1>{utente.display_name}</h1>
      <div>
        <span class="ruolo">{NOMI_RUOLO[utente.role].toUpperCase()}</span>
        {#if classe}<span>Classe {classe}</span>{/if}
      </div>
    </div>
  </section>

  <dl class="scheda-dati">
    <dt>Utente</dt><dd class="mono">{utente.username}</dd>
    {#if admin}
      <dt>Compito</dt><dd>Gestione del server. Non fa parte della classe.</dd>
    {:else}
      <dt>Gruppi</dt><dd class:tenue={!mieiGruppi.length}>{mieiGruppi.map((g) => g.name).join(' · ') || 'Nessuno'}</dd>
      <dt>Compleanno</dt>
      <dd class:tenue={!profilo?.compleanno}>
        {compleanno}{#if profilo === null}<button type="button" class="sblocca" onclick={() => (sblocco = true)}>Sblocca</button>{/if}
      </dd>
    {/if}
  </dl>

  <nav aria-label="Impostazioni">
    {#snippet riga(href: string, titolo: string, valore: string)}
      <a {href}><span class="serif">{titolo}</span><span class="valore">{valore}<Icona nome="destra" size={16} /></span></a>
    {/snippet}
    {@render riga('#/aspetto', 'Aspetto e tema', nomeTema)}
    {#if !admin}{@render riga('#/aspetto', 'Notifiche', 'In arrivo')}{/if}
    {@render riga('#/plugin', 'Plugin', pluginAttivi ? '1 attivo' : 'Nessuno attivo')}
    {#if admin}
      {@render riga('#/persone', 'Persone e gruppi', 'Admin')}
      {@render riga('#/backup', 'Backup', 'Admin')}
    {/if}
    <button type="button" class="voce-menu" onclick={() => (foglio = 'privacy')}>
      <span class="serif">Privacy e sicurezza</span>
      <span class="valore">{utente.sessions} {utente.sessions === 1 ? 'dispositivo' : 'dispositivi'}<Icona nome="destra" size={16} /></span>
    </button>
    <button type="button" class="voce-menu esci" onclick={esci}><span class="serif">Esci</span></button>
  </nav>

  <p class="piede">TaskHit 0.1 · {location.host}<br />
    Ultima sincronizzazione: {app.ultimaSync ? momento(app.ultimaSync / 1000) : 'mai'} · {app.online ? 'dati disponibili offline' : 'sei offline'}</p>
</div>

<Sblocco bind:aperto={sblocco} riga={false} />

{#if foglio === 'modifica'}
  <Foglio titolo="Modifica profilo" onclose={() => (foglio = '')}>
    <form onsubmit={salva}>
      <label class="campo"><span class="dato">Nome visualizzato</span><input bind:value={nome} required maxlength="60" autocomplete="name" /></label>
      {#if !admin}
        {#if app.sbloccato && profilo !== null}
          <label class="campo"><span class="dato">Compleanno · cifrato</span><input type="date" bind:value={data} /></label>
        {:else}
          <p class="nota">Il compleanno è cifrato: <button type="button" class="sblocca" onclick={() => { foglio = ''; sblocco = true; }}>sblocca</button> per modificarlo.</p>
        {/if}
      {/if}
      {#if errore}<p class="errore" role="alert">{errore}</p>{/if}
      <button class="pulsante pieno grande" disabled={occupato} aria-busy={occupato}>Salva</button>
    </form>
  </Foglio>
{:else if foglio === 'privacy'}
  <Foglio titolo="Privacy e sicurezza" onclose={() => (foglio = '')}>
    <dl class="scheda-dati">
      {#if !admin}
        <dt>Privati</dt><dd>Cifrati sul dispositivo: il server non li legge.</dd>
        <dt>Stato</dt><dd>{app.sbloccato ? 'Sbloccati in questa sessione' : 'Bloccati'}{#if !app.sbloccato}<button type="button" class="sblocca" onclick={() => { foglio = ''; sblocco = true; }}>Sblocca</button>{/if}</dd>
      {/if}
      <dt>Accessi</dt><dd>{utente.sessions} {utente.sessions === 1 ? 'dispositivo collegato' : 'dispositivi collegati'}</dd>
    </dl>
    <button type="button" class="pulsante grande" disabled={utente.sessions < 2 || !app.online} onclick={esciAltrove}>Esci dagli altri dispositivi</button>
    <p class="nota">Il cambio password arriva in una prossima versione.</p>
  </Foglio>
{/if}

<style>
  .testa { display: flex; align-items: center; justify-content: space-between; }
  .testa .dato { font-size: 11px; letter-spacing: .1em; }
  .identita { display: flex; align-items: flex-end; gap: 18px; padding-bottom: 18px; border-bottom: 1px solid var(--inchiostro); }
  .foto { position: relative; flex: none; display: flex; }
  .foto :global(> span) { font-family: var(--font-titoli); font-weight: 400; font-size: 46px !important; }
  .foto button { position: absolute; right: -4px; bottom: -4px; width: 44px; height: 44px; border-radius: 50%; border: 3px solid var(--sfondo); background: var(--inchiostro); color: var(--sfondo); display: flex; align-items: center; justify-content: center; opacity: 1; }
  .chi { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  /* Nome e cognome su due righe, come in una testata. */
  h1 { width: min-content; max-width: 100%; font-size: 46px; line-height: .9; overflow-wrap: break-word; }
  .chi div { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; font-size: 13px; color: var(--inchiostro-2); }
  .ruolo { min-height: 24px; padding: 0 8px; display: inline-flex; align-items: center; background: var(--accento); color: var(--su-accento); font-family: var(--font-dati); font-size: 10px; letter-spacing: .08em; }
  dd.mono { font-size: 14px; }
  .sblocca { margin-left: 8px; font-size: 13px; color: var(--accento); text-decoration: underline; text-underline-offset: 3px; }
  nav { display: flex; flex-direction: column; }
  nav a, .voce-menu { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 52px; border-bottom: 1px solid var(--filetto); text-align: left; }
  nav .serif { font-size: 24px; }
  .valore { display: flex; align-items: center; gap: 6px; font-size: 13px; color: var(--attenuato); }
  .esci { color: var(--attenuato); }
  .piede { font-family: var(--font-dati); font-size: 10.5px; letter-spacing: .04em; color: var(--attenuato); line-height: 1.6; overflow-wrap: anywhere; }
  form { display: flex; flex-direction: column; gap: 18px; }
</style>
