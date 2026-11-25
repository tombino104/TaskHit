<script lang="ts">
  import Foglio from './Foglio.svelte';
  import { api, type Gruppo } from '../api';
  import { messaggio, sincronizza } from '../store.svelte';

  let { onclose, onfatto }: { onclose: () => void; onfatto?: () => unknown } = $props();
  let nome = $state('');
  let tipo = $state<Gruppo['kind']>('classe');
  let errore = $state('');
  let occupato = $state(false);

  async function crea(ev: SubmitEvent) {
    ev.preventDefault(); errore = ''; occupato = true;
    try {
      await api.creaGruppo(nome, tipo);
      await sincronizza();
      await onfatto?.();
      onclose();
    } catch (e) { errore = messaggio(e); } finally { occupato = false; }
  }
</script>

<Foglio titolo="Nuovo gruppo" {onclose}>
  <form onsubmit={crea}>
    <label class="campo"><span class="dato">Nome</span><input bind:value={nome} required maxlength="40" placeholder="es. 4ªB" /></label>
    <div role="radiogroup" aria-label="Tipo" class="segmenti">
      <button type="button" role="radio" aria-checked={tipo === 'classe'} onclick={() => (tipo = 'classe')}>Classe</button>
      <button type="button" role="radio" aria-checked={tipo === 'gruppo'} onclick={() => (tipo = 'gruppo')}>Gruppo</button>
    </div>
    {#if errore}<p class="errore" role="alert">{errore}</p>{/if}
    <button class="pulsante pieno grande" disabled={occupato} aria-busy={occupato}>Crea</button>
  </form>
</Foglio>

<style>
  form { display: flex; flex-direction: column; gap: 18px; }
</style>
