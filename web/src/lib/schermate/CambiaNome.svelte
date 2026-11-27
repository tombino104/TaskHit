<script lang="ts">
  import { t } from '../i18n';
  // Passo obbligatorio: completare il nome (account creati prima) o cambiarlo su richiesta di chi modera.
  import { app, esci, messaggio, salvaNome } from '../store.svelte';
  import { nomiMotivi } from '../moderazione';

  const richiesta = $derived(app.utente?.rename_request);
  let nome = $state('');
  let cognome = $state('');
  let errore = $state('');
  let occupato = $state(false);

  async function salva(ev: SubmitEvent) {
    ev.preventDefault(); errore = ''; occupato = true;
    try { await salvaNome(nome, cognome); } catch (e) { errore = messaggio(e); } finally { occupato = false; }
  }
</script>

<div class="schermata nome">
  <span class="marchio">TaskHit</span>
  <span class="dato">{t('Passo obbligatorio')}</span>
  <h1>{richiesta ? t('Cambia il tuo nome') : t('Come ti chiami?')}</h1>
  {#if richiesta}
    <p class="errore">{t('Ti è stato chiesto di cambiare nome')}{richiesta.tags.length ? `: ${nomiMotivi(richiesta.tags)}` : ''}.{richiesta.reason ? ` «${richiesta.reason}»` : ''}</p>
  {/if}
  <p class="lede">{t('D’ora in poi si entra con')} <strong>{t('nome, cognome e password')}</strong>{t('. La password resta la stessa.')}</p>
  <form onsubmit={salva}>
    <label class="campo"><span class="dato">{t('Nome')}</span><input bind:value={nome} required maxlength="40" autocomplete="given-name" /></label>
    <label class="campo"><span class="dato">{t('Cognome')}</span><input bind:value={cognome} required maxlength="40" autocomplete="family-name" /></label>
    {#if errore}<p class="errore" role="alert">{errore}</p>{/if}
    <button class="pulsante pieno grande" disabled={occupato || !app.online} aria-busy={occupato}>{t('Salva')}</button>
  </form>
  <button type="button" class="collegamento" onclick={esci}>{t('Esci')}</button>
</div>

<style>
  .nome { max-width: 480px; }
  h1 { font-size: 44px; line-height: .98; text-wrap: balance; }
  form { display: flex; flex-direction: column; gap: 18px; }
  .collegamento { align-self: center; }
</style>
