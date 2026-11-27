<script lang="ts">
  import { t } from '../i18n';
  import Icona from '../ui/Icona.svelte';
  import Interruttore from '../ui/Interruttore.svelte';
  import Lingue from '../ui/Lingue.svelte';
  import { app, caricaTemi, scegliPreferenze } from '../store.svelte';
  import { pickTheme, tipoDi, type Modo } from '../theme';
  import { onMount } from 'svelte';

  onMount(() => void caricaTemi()); // un file aggiunto nella cartella compare subito

  const MODI: [Modo, string][] = [['chiaro', t('Chiaro')], ['scuro', t('Scuro')], ['sistema', t('Sistema')]];
  const scelto = $derived(pickTheme(app.temi, app.preferenze));
  const buio = matchMedia('(prefers-color-scheme: dark)');
  const nota = $derived.by(() => {
    if (app.preferenze.modo === 'sistema') return t('Segue il dispositivo: {tema} chiaro di giorno, scuro quando il dispositivo passa al buio.', { tema: scelto?.nome ?? t('il tema') });
    return t('Sempre {versione}, qualunque sia l’impostazione del dispositivo.', { versione: t(tipoDi(app.preferenze, buio.matches)) });
  });
  const VERSIONI = ['chiaro', 'scuro'] as const;

  const NOTIFICHE = [
    [t('Nuovi eventi pubblici'), t('Quando qualcuno aggiunge un evento per la classe')],
    [t('Modifiche ed eliminazioni'), t('Date, orari o eventi annullati')],
    [t('Promemoria'), t('Gli avvisi che imposti su ogni evento')],
    [t('Proposte da approvare'), t('Solo per i rappresentanti')],
    [t('Non disturbare'), t('Silenzia tutto dalle 22:00 alle 07:00')],
  ] as const;
</script>

<div class="schermata aspetto">
  <a href="#/profilo" class="torna"><Icona nome="sinistra" size={20} />{t('Profilo')}</a>
  <h1 class="titolo sottolineato">{t('Aspetto')}{#if app.utente?.role !== 'admin'}{' '}<em>&amp;</em> {t('notifiche')}{/if}</h1>

  <section>
    <h2 class="dato">{t('Lingua')}</h2>
    <Lingue />
  </section>

  <section>
    <h2 class="dato">{t('Modalità')}</h2>
    <div role="radiogroup" aria-label={t('Modalità colore')} class="segmenti">
      {#each MODI as [id, nome] (id)}
        <button type="button" role="radio" aria-checked={app.preferenze.modo === id} onclick={() => scegliPreferenze({ modo: id })}>{nome}</button>
      {/each}
    </div>
    <p class="nota">{nota}</p>
  </section>

  <section>
    <h2 class="dato">{t('Temi installati')}</h2>
    <div role="radiogroup" aria-label={t('Tema')} class="temi">
      {#each app.temi as tema (tema.id)}
        <button type="button" role="radio" aria-checked={scelto?.id === tema.id} onclick={() => scegliPreferenze({ tema: tema.id })}>
          <span class="anteprima" aria-hidden="true">
            {#each VERSIONI as v (v)}
              <span class="meta" style:background={tema[v].sfondo}>
                <span style:color={tema[v].inchiostro} style:font-family={tema.font?.titoli ? `'${tema.font.titoli}', serif` : undefined}>Aa</span>
                <span class="segni"><i style:background={tema[v].inchiostro}></i><i style:background={tema[v].accento}></i></span>
              </span>
            {/each}
          </span>
          <span class="nome"><span class="serif">{tema.nome}</span></span>
        </button>
      {/each}
    </div>
    <button type="button" class="pulsante tratteggiato" disabled><Icona nome="carica" size={18} />{t('Carica un tema (.json)')}</button>
    <p class="nota">{t('Un tema è un solo file con font e colori, chiari e scuri. Il caricamento da qui è in arrivo: per ora l\'admin copia il file nella cartella dei temi del server, e chiunque può sceglierlo.')}</p>
  </section>

  {#if app.utente?.role !== 'admin'}
  <section class="notifiche">
    <h2 class="dato">{t('Notifiche push · in arrivo')}</h2>
    {#each NOTIFICHE as [nome, descrizione] (nome)}
      <Interruttore {nome} {descrizione} attivo={false} disabilitato />
    {/each}
    <button type="button" class="pulsante prova" disabled>{t('Invia una notifica di prova')}</button>
  </section>
  {/if}
</div>

<style>
  .aspetto { gap: 22px; }
  .titolo { margin-top: -8px; font-size: 50px; line-height: .95; }
  section { display: flex; flex-direction: column; gap: 12px; }
  .segmenti > * { min-height: 44px; font-size: 14px; }
  .temi { display: grid; grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); gap: 12px; }
  .temi button { display: flex; flex-direction: column; gap: 8px; padding: 8px 8px 10px; border: 1px solid var(--filetto); text-align: left; }
  .temi [aria-checked='true'] { border: 1.5px solid var(--inchiostro); }
  .anteprima { display: grid; grid-template-columns: 1fr 1fr; height: 72px; border: 1px solid rgba(127, 127, 127, .25); font-family: var(--font-titoli); font-size: 22px; line-height: 1; }
  .meta { display: flex; flex-direction: column; justify-content: space-between; padding: 10px; min-width: 0; }
  .segni { display: flex; align-items: center; gap: 6px; }
  .segni i:first-child { height: 3px; width: 46%; }
  .segni i:last-child { width: 10px; height: 10px; border-radius: 50%; }
  .nome { display: flex; align-items: baseline; justify-content: space-between; gap: 6px; }
  .nome .serif { font-size: 22px; line-height: 1; overflow-wrap: anywhere; }
  .notifiche { gap: 0; }
  .notifiche h2 { margin-bottom: 4px; }
  .prova { margin-top: 16px; font-size: 14px; }
</style>
