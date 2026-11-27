<script lang="ts">
  import Foglio from './Foglio.svelte';
  import { api, type Gruppo } from '../api';
  import { app, messaggio, sincronizza } from '../store.svelte';

  let { onclose, onfatto }: { onclose: () => void; onfatto?: () => unknown } = $props();
  let nome = $state('');
  let tipo = $state<Gruppo['kind']>('classe');
  // Il rappresentante crea solo gruppi, dentro una delle sue classi.
  const admin = $derived(app.utente?.role === 'admin');
  const classi = $derived(app.meta?.groups.filter((g) => g.kind === 'classe' && g.manage) ?? []);
  let scelta = $state('');
  const classe = $derived(scelta || classi[0]?.id);
  let errore = $state('');
  let occupato = $state(false);

  async function crea(ev: SubmitEvent) {
    ev.preventDefault(); errore = ''; occupato = true;
    try {
      await api.creaGruppo(nome, admin ? tipo : 'gruppo', admin ? undefined : classe);
      await sincronizza();
      await onfatto?.();
      onclose();
    } catch (e) { errore = messaggio(e); } finally { occupato = false; }
  }
</script>

<Foglio titolo="Nuovo gruppo" {onclose}>
  <form onsubmit={crea}>
    <label class="campo"><span class="dato">Nome</span><input bind:value={nome} required maxlength="40" placeholder={admin ? 'es. 4ªB' : 'es. 4ªB laboratorio'} /></label>
    {#if admin}
      <div role="radiogroup" aria-label="Tipo" class="segmenti">
        <button type="button" role="radio" aria-checked={tipo === 'classe'} onclick={() => (tipo = 'classe')}>Classe</button>
        <button type="button" role="radio" aria-checked={tipo === 'gruppo'} onclick={() => (tipo = 'gruppo')}>Gruppo</button>
      </div>
    {:else if classi.length > 1}
      <label class="campo"><span class="dato">Classe</span>
        <select value={classe} onchange={(e) => (scelta = e.currentTarget.value)}>{#each classi as c (c.id)}<option value={c.id}>{c.name}</option>{/each}</select>
      </label>
    {:else}
      <p class="nota">{classi[0] ? `Il gruppo nasce dentro ${classi[0].name}.` : 'Serve far parte di una classe.'}</p>
    {/if}
    {#if errore}<p class="errore" role="alert">{errore}</p>{/if}
    <button class="pulsante pieno grande" disabled={occupato || (!admin && !classe)} aria-busy={occupato}>Crea</button>
  </form>
</Foglio>

<style>
  form { display: flex; flex-direction: column; gap: 18px; }
</style>
