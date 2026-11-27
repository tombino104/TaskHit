<script lang="ts">
  import { t } from '../i18n';
  import Foglio from '../ui/Foglio.svelte';
  import { app } from '../store.svelte';
  import { commuta, SENZA_FILTRI, type Filtri, type Visibilita } from '../events';

  let { onclose, conta }: { onclose: () => void; conta: (f: Filtri) => number } = $props();
  const n = $derived(conta(app.filtri));
  const VISIBILITA: [Visibilita, string][] = [['tutti', t('Tutti')], ['pubblici', t('Pubblici')], ['privati', t('Privati')]];
</script>

<Foglio titolo={t('Filtra')} {onclose}>
  {#snippet azione()}
    <button type="button" class="collegamento" onclick={() => (app.filtri = { ...SENZA_FILTRI })}>{t('Azzera')}</button>
  {/snippet}

  <fieldset>
    <legend class="dato">{t('Tipo di evento')}</legend>
    <div class="scelte">
      {#each app.meta?.labels.filter((x) => !x.hidden) ?? [] as tipo (tipo.id)}
        <button type="button" class="scelta tipo" aria-pressed={app.filtri.tipi.includes(tipo.id)} onclick={() => (app.filtri.tipi = commuta(app.filtri.tipi, tipo.id))}>{t(tipo.name)}</button>
      {/each}
    </div>
  </fieldset>

  <fieldset>
    <legend class="dato">{t('Materie')}</legend>
    <div class="materie">
      {#each app.meta?.tags ?? [] as m (m.id)}
        <label>
          <input type="checkbox" checked={app.filtri.materie.includes(m.id)} onchange={() => (app.filtri.materie = commuta(app.filtri.materie, m.id))} />
          <i class="quadratino grande" style:background={m.color}></i>
          <span>{m.name}</span>
        </label>
      {/each}
    </div>
  </fieldset>

  <fieldset>
    <legend class="dato">{t('Visibilità')}</legend>
    <div role="radiogroup" aria-label={t('Visibilità')} class="segmenti">
      {#each VISIBILITA as [id, nome] (id)}
        <button type="button" role="radio" aria-checked={app.filtri.visibilita === id} onclick={() => (app.filtri.visibilita = id)}>{nome}</button>
      {/each}
    </div>
  </fieldset>

  <button type="button" class="pulsante pieno grande" onclick={onclose}>{n === 1 ? t('Mostra 1 evento') : t('Mostra {n} eventi', { n })}</button>
</Foglio>

<style>
  legend { padding: 0 0 10px; }
  .materie { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); column-gap: 16px; }
  label { display: flex; align-items: center; gap: 10px; min-height: 44px; border-bottom: 1px solid var(--filetto); font-size: 14.5px; cursor: pointer; }
</style>
