<script lang="ts">
  import { t } from '../i18n';
  import Icona from '../ui/Icona.svelte';
  import RigaEvento from '../ui/RigaEvento.svelte';
  import Sblocco from '../ui/Sblocco.svelte';
  import { app } from '../store.svelte';
  import { indietro } from '../router.svelte';
  import { perGiorno, riassunto } from '../events';
  import { daChiave, dataLunga } from '../dates';

  let testo = $state('');
  const trovati = $derived.by(() => {
    const q = testo.trim().toLowerCase();
    if (q.length < 2) return [];
    return [...perGiorno(app.eventi.filter((e) => `${e.title} ${e.description ?? ''} ${riassunto(app.meta, e)}`.toLowerCase().includes(q)))];
  });
  const fuoco = (el: HTMLInputElement) => el.focus();
</script>

<div class="schermata con-menu">
  <button type="button" class="torna" onclick={() => indietro()}><Icona nome="sinistra" size={20} />{t('Indietro')}</button>
  <h1 class="titolo sottolineato">{t('Cerca')}</h1>
  <label class="campo"><span class="dato">{t('Titolo, argomento o materia')}</span><input type="search" bind:value={testo} {@attach fuoco} /></label>
  <Sblocco />
  {#each trovati as [giorno, eventi] (giorno)}
    <section>
      <h2 class="dato occhiello"><span>{dataLunga(daChiave(giorno))} {daChiave(giorno).getFullYear()}</span><span>{eventi.length}</span></h2>
      {#each eventi as e (e.id)}<RigaEvento evento={e} />{/each}
    </section>
  {:else}
    <p class="nota">{testo.trim().length < 2 ? t('Scrivi almeno due lettere.') : t('Nessun evento trovato.')}</p>
  {/each}
</div>
