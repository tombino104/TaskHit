<script lang="ts">
  // Avviso e pannello per riaprire gli eventi privati: la chiave sta solo in memoria.
  import Foglio from './Foglio.svelte';
  import Icona from './Icona.svelte';
  import { app, avvisa, messaggio, sblocca } from '../store.svelte';

  let { aperto = $bindable(false), riga = true }: { aperto?: boolean; riga?: boolean } = $props();
  let password = $state('');
  let errore = $state('');
  let occupato = $state(false);

  async function invia(ev: SubmitEvent) {
    ev.preventDefault();
    errore = ''; occupato = true;
    try {
      await sblocca(password);
      password = ''; aperto = false; avvisa('Eventi privati sbloccati.');
    } catch (e) { errore = messaggio(e); } finally { occupato = false; }
  }
</script>

{#if riga && !app.sbloccato && app.bloccati}
  <button type="button" class="riga" onclick={() => (aperto = true)}>
    <Icona nome="lucchetto" size={16} />
    <span>{app.bloccati === 1 ? 'Un evento privato bloccato' : `${app.bloccati} eventi privati bloccati`}</span>
    <span class="sblocca">Sblocca</span>
  </button>
{/if}

{#if aperto}
  <Foglio titolo="Sblocca" onclose={() => { aperto = false; password = ''; errore = ''; }}>
    <p class="lede">I tuoi eventi privati sono cifrati. Scrivi la password per leggerli su questo dispositivo: resta aperto finché non chiudi l'app.</p>
    <form onsubmit={invia}>
      <label class="campo"><span class="dato">Password</span><input type="password" bind:value={password} required autocomplete="current-password" /></label>
      {#if errore}<p class="errore" role="alert">{errore}</p>{/if}
      <button class="pulsante pieno grande" disabled={occupato} aria-busy={occupato}>{occupato ? 'Derivo le chiavi…' : 'Sblocca'}</button>
    </form>
  </Foglio>
{/if}

<style>
  .riga { display: flex; align-items: center; gap: 8px; width: 100%; min-height: 48px; padding: 0 12px; border: 1px dashed var(--attenuato); font-size: 13.5px; text-align: left; }
  .sblocca { margin-left: auto; color: var(--accento); text-decoration: underline; text-underline-offset: 3px; }
  form { display: flex; flex-direction: column; gap: 18px; }
</style>
