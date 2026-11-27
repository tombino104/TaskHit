<script lang="ts">
  import { t } from './lib/i18n';
  import { onMount } from 'svelte';
  import { api, type Kdf } from './lib/api';
  import { app, caricaTemi, messaggio, riapplicaTema, riprendi, sincronizza } from './lib/store.svelte';
  import { rotta, sostituisci } from './lib/router.svelte';
  import Barra from './lib/ui/Barra.svelte';
  import Menu from './lib/ui/Menu.svelte';
  import Accesso from './lib/schermate/Accesso.svelte';
  import Setup from './lib/schermate/Setup.svelte';
  import Oggi from './lib/schermate/Oggi.svelte';
  import Calendario from './lib/schermate/Calendario.svelte';
  import Evento from './lib/schermate/Evento.svelte';
  import ModificaEvento from './lib/schermate/ModificaEvento.svelte';
  import Inbox from './lib/schermate/Inbox.svelte';
  import Approvazioni from './lib/schermate/Approvazioni.svelte';
  import Profilo from './lib/schermate/Profilo.svelte';
  import Aspetto from './lib/schermate/Aspetto.svelte';
  import Plugin from './lib/schermate/Plugin.svelte';
  import Persone from './lib/schermate/Persone.svelte';
  import Backup from './lib/schermate/Backup.svelte';
  import Cerca from './lib/schermate/Cerca.svelte';
  import Lavori from './lib/schermate/Lavori.svelte';
  import Classe from './lib/schermate/Classe.svelte';
  import CambiaNome from './lib/schermate/CambiaNome.svelte';
  import ComponiLavori from './lib/schermate/ComponiLavori.svelte';
  import ServeHttps from './lib/schermate/ServeHttps.svelte';

  /** Ogni quanto si ricontrolla il server mentre l'app è visibile. */
  const INTERVALLO = 60_000;

  /** In HTTP, fuori da localhost, manca WebCrypto: l'app non può funzionare. */
  const cifraturaDisponibile = globalThis.crypto?.subtle !== undefined;

  let fase = $state<'caricamento' | 'setup' | 'pronta'>('caricamento');
  let kdf = $state<Kdf | null>(null);
  let errore = $state('');

  async function aggiorna() {
    if (fase !== 'pronta' || !app.utente || document.visibilityState !== 'visible') return;
    try { await sincronizza(); } catch { /* si riprova al giro successivo */ }
  }

  onMount(() => {
    const su = () => { app.online = true; void aggiorna(); };
    const giu = () => (app.online = false);
    const scuro = matchMedia('(prefers-color-scheme: dark)');
    addEventListener('online', su); addEventListener('offline', giu);
    document.addEventListener('visibilitychange', aggiorna);
    scuro.addEventListener('change', riapplicaTema);
    const timer = setInterval(aggiorna, INTERVALLO);
    (async () => {
      await caricaTemi();
      if (!cifraturaDisponibile) return;
      try {
        const st = await api.setupStatus();
        kdf = st.kdf;
        if (st.needs_setup) { fase = 'setup'; return; }
      } catch (e) {
        errore = messaggio(e); // offline: si prova con la copia sul dispositivo
      }
      await riprendi();
      fase = 'pronta';
    })();
    return () => {
      removeEventListener('online', su); removeEventListener('offline', giu);
      document.removeEventListener('visibilitychange', aggiorna);
      scuro.removeEventListener('change', riapplicaTema);
      clearInterval(timer);
    };
  });

  // Il parametro di derivazione serve anche se l'app è partita offline.
  $effect(() => {
    if (fase === 'pronta' && !app.utente && !kdf && app.online) api.setupStatus().then((s) => (kdf = s.kdf)).catch(() => {});
  });

  // L'admin gestisce l'impianto e non ha un calendario: le due parti dell'app non si sovrappongono.
  const COMUNI = ['profilo', 'aspetto', 'plugin'];
  const DELLA_CLASSE = [...COMUNI, 'oggi', 'calendario', 'evento', 'nuovo', 'modifica', 'inbox', 'approvazioni', 'cerca', 'lavori', 'classe'];
  const DELL_ADMIN = [...COMUNI, 'persone', 'backup'];
  const admin = $derived(app.utente?.role === 'admin');
  const permessa = $derived((admin ? DELL_ADMIN : DELLA_CLASSE).includes(rotta.nome)
    || (rotta.nome === 'persone' && app.utente?.role === 'rappresentante'));
  const conMenu = $derived((admin ? ['persone', 'backup', 'profilo'] : ['oggi', 'calendario', 'inbox', 'profilo', 'cerca', 'lavori', 'classe']).includes(rotta.nome)
    && !(rotta.nome === 'lavori' && rotta.arg));
  $effect(() => { if (fase === 'pronta' && app.utente && !permessa) sostituisci(admin ? 'persone' : 'oggi'); });
</script>

{#if !cifraturaDisponibile}
  <main><ServeHttps /></main>
{:else if fase === 'caricamento'}
  <main class="schermata"><span class="marchio">TaskHit</span><p class="dato" role="status">{t('Caricamento…')}</p></main>
{:else if fase === 'setup'}
  <main><Setup {kdf} onfine={() => { sostituisci('oggi'); fase = 'pronta'; }} /></main>
{:else if !app.utente}
  <main><Accesso {kdf} {errore} /></main>
{:else if app.meta && (app.utente.rename_request || app.utente.first_name === null)}
  <main><CambiaNome /></main>
{:else if app.meta}
  <div class="app">
    <Barra />
    <main>
      {#if !permessa}<!-- si sta passando alla pagina giusta -->
      {:else if rotta.nome === 'oggi'}<Oggi />
      {:else if rotta.nome === 'calendario'}<Calendario />
      {:else if rotta.nome === 'evento'}<Evento />
      {:else if rotta.nome === 'nuovo' || rotta.nome === 'modifica'}
        <!-- Si ricrea anche quando l'evento da modificare diventa leggibile (dopo lo sblocco). -->
        {#key `${rotta.nome}/${rotta.arg}/${app.eventi.some((e) => e.id === rotta.arg)}`}<ModificaEvento />{/key}
      {:else if rotta.nome === 'inbox'}<Inbox />
      {:else if rotta.nome === 'approvazioni'}<Approvazioni />
      {:else if rotta.nome === 'profilo'}<Profilo />
      {:else if rotta.nome === 'aspetto'}<Aspetto />
      {:else if rotta.nome === 'plugin'}<Plugin />
      {:else if rotta.nome === 'persone'}<Persone />
      {:else if rotta.nome === 'backup'}<Backup />
      {:else if rotta.nome === 'cerca'}<Cerca />
      {:else if rotta.nome === 'classe'}<Classe />
      {:else if rotta.nome === 'lavori' && rotta.arg && app.utente.role === 'rappresentante'}
        {#key rotta.arg}<ComponiLavori quale={rotta.arg} />{/key}
      {:else if rotta.nome === 'lavori'}<Lavori />
      {/if}
    </main>
    {#if conMenu}<Menu />{/if}
  </div>
{/if}

<div class="avviso" role="status" aria-live="polite">{#if app.avviso}<p>{app.avviso}</p>{/if}</div>

<style>
  .app { display: flex; min-height: 100dvh; }
  .app main { flex: 1; min-width: 0; }
  .avviso { position: fixed; z-index: 30; left: 16px; right: 16px; bottom: calc(96px + var(--basso)); display: flex; justify-content: center; pointer-events: none; }
  .avviso p { max-width: 480px; padding: 12px 16px; background: var(--inchiostro); color: var(--sfondo); font-size: 14px; overflow-wrap: anywhere; pointer-events: auto; }
</style>
