<script lang="ts">
  // Accesso e richiesta di iscrizione con codice invito (#/entra/CODICE).
  import ChiaveRecupero from '../ui/ChiaveRecupero.svelte';
  import { api, ApiError, type Kdf } from '../api';
  import { accedi, app, messaggio, preparaChiavi } from '../store.svelte';
  import { rotta, sostituisci } from '../router.svelte';

  let { kdf, errore: iniziale = '' }: { kdf: Kdf | null; errore?: string } = $props();

  const entra = $derived(rotta.nome === 'entra');
  let username = $state('');
  let nome = $state('');
  let codice = $state('');
  let password = $state('');
  let password2 = $state('');
  let errore = $state('');
  let occupato = $state(false);
  let chiave = $state('');
  let inviata = $state(false);
  $effect(() => { if (rotta.nome === 'entra' && rotta.arg) codice = rotta.arg; });
  $effect(() => { errore = iniziale; });

  async function login(ev: SubmitEvent) {
    ev.preventDefault(); errore = ''; occupato = true;
    try {
      await accedi(username, password);
      password = '';
      sostituisci('oggi');
    } catch (e) { errore = e instanceof ApiError && e.status === 401 ? 'Nome utente o password errati.' : messaggio(e); }
    finally { occupato = false; }
  }

  async function iscriviti(ev: SubmitEvent) {
    ev.preventDefault(); errore = '';
    if (password.length < 10) { errore = 'La password deve avere almeno 10 caratteri.'; return; }
    if (password !== password2) { errore = 'Le password non coincidono.'; return; }
    occupato = true;
    try {
      const k = await preparaChiavi(password, kdf ?? undefined);
      await api.join({ code: codice, username, display_name: nome || username, ...k.account });
      k.dek.fill(0);
      password = password2 = '';
      chiave = k.recupero; inviata = true;
    } catch (e) { errore = messaggio(e); } finally { occupato = false; }
  }
</script>

{#if chiave}
  <ChiaveRecupero {chiave} onfatto={() => (chiave = '')} />
{:else if inviata}
  <div class="schermata">
    <span class="marchio">TaskHit</span>
    <span class="dato">Richiesta inviata</span>
    <h1>Manca solo la conferma</h1>
    <p class="lede">L'admin deve accettare la tua richiesta. Dopo potrai entrare con nome utente e password.</p>
    <button type="button" class="pulsante pieno grande" onclick={() => { inviata = false; sostituisci('accesso'); }}>Vai all'accesso</button>
  </div>
{:else}
  <div class="schermata accesso">
    <div class="testa">
      <span class="marchio">TaskHit</span>
      <span class="dato">{app.online ? (entra ? 'Invito' : 'Accesso') : 'Offline'}</span>
    </div>
    <header>
      <h1>{entra ? 'Entra nella classe' : 'Bentornato'}</h1>
      <p class="lede">{entra ? 'Con il codice invito chiedi di entrare come alunno. La password non lascia mai questo dispositivo.' : app.meta?.settings.instance_name || 'Il calendario condiviso della classe.'}</p>
    </header>

    <form onsubmit={entra ? iscriviti : login}>
      {#if entra}
        <label class="campo"><span class="dato">Codice invito</span><input class="mono" bind:value={codice} required autocapitalize="characters" autocomplete="off" spellcheck="false" placeholder="XXX-XXX-XXX" /></label>
        <label class="campo"><span class="dato">Nome e cognome</span><input bind:value={nome} required maxlength="60" autocomplete="name" /></label>
      {/if}
      <label class="campo"><span class="dato">Nome utente</span><input class="mono" bind:value={username} required minlength="3" maxlength="32" autocomplete="username" autocapitalize="none" spellcheck="false" /></label>
      <label class="campo"><span class="dato">Password</span><input type="password" bind:value={password} required autocomplete={entra ? 'new-password' : 'current-password'} placeholder={entra ? 'Almeno 10 caratteri' : ''} /></label>
      {#if entra}
        <label class="campo"><span class="dato">Ripeti password</span><input type="password" bind:value={password2} required autocomplete="new-password" /></label>
      {/if}
      {#if errore}<p class="errore" role="alert">{errore}</p>{/if}
      <button class="pulsante pieno grande" disabled={occupato || !app.online} aria-busy={occupato}>
        {occupato ? 'Derivo le chiavi…' : entra ? 'Chiedi di entrare' : 'Entra'}
      </button>
    </form>

    {#if entra}
      <a href="#/accesso" class="collegamento">Ho già un account</a>
    {:else}
      <a href="#/entra" class="collegamento">Hai un codice invito?</a>
    {/if}
  </div>
{/if}

<style>
  .accesso, .schermata { max-width: 480px; }
  .testa { display: flex; align-items: baseline; justify-content: space-between; border-bottom: 1px solid var(--inchiostro); padding-bottom: 12px; }
  header { display: flex; flex-direction: column; gap: 8px; padding-top: 12px; }
  h1 { font-size: 44px; line-height: .98; text-wrap: balance; }
  form { display: flex; flex-direction: column; gap: 18px; }
  form .pulsante { margin-top: 6px; }
  .collegamento { align-self: center; }
</style>
