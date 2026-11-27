<script lang="ts">
  import Icona from '../ui/Icona.svelte';
  import Interruttore from '../ui/Interruttore.svelte';
  import { app, caricaTemi, scegliPreferenze } from '../store.svelte';
  import { pickTheme, tipoDi, type Modo } from '../theme';
  import { onMount } from 'svelte';

  onMount(() => void caricaTemi()); // un file aggiunto nella cartella compare subito

  const MODI: [Modo, string][] = [['chiaro', 'Chiaro'], ['scuro', 'Scuro'], ['sistema', 'Sistema']];
  const scelto = $derived(pickTheme(app.temi, app.preferenze));
  const buio = matchMedia('(prefers-color-scheme: dark)');
  const nota = $derived.by(() => {
    if (app.preferenze.modo === 'sistema') return `Segue il dispositivo: ${scelto?.nome ?? 'il tema'} chiaro di giorno, scuro quando il dispositivo passa al buio.`;
    return `Sempre ${tipoDi(app.preferenze, buio.matches)}, qualunque sia l’impostazione del dispositivo.`;
  });
  const VERSIONI = ['chiaro', 'scuro'] as const;

  const NOTIFICHE = [
    ['Nuovi eventi pubblici', 'Quando qualcuno aggiunge un evento per la classe'],
    ['Modifiche ed eliminazioni', 'Date, orari o eventi annullati'],
    ['Promemoria', 'Gli avvisi che imposti su ogni evento'],
    ['Proposte da approvare', 'Solo per i rappresentanti'],
    ['Non disturbare', 'Silenzia tutto dalle 22:00 alle 07:00'],
  ] as const;
</script>

<div class="schermata aspetto">
  <a href="#/profilo" class="torna"><Icona nome="sinistra" size={20} />Profilo</a>
  <h1 class="titolo sottolineato">Aspetto{#if app.utente?.role !== 'admin'}{' '}<em>&amp;</em> notifiche{/if}</h1>

  <section>
    <h2 class="dato">Modalità</h2>
    <div role="radiogroup" aria-label="Modalità colore" class="segmenti">
      {#each MODI as [id, nome] (id)}
        <button type="button" role="radio" aria-checked={app.preferenze.modo === id} onclick={() => scegliPreferenze({ modo: id })}>{nome}</button>
      {/each}
    </div>
    <p class="nota">{nota}</p>
  </section>

  <section>
    <h2 class="dato">Temi installati</h2>
    <div role="radiogroup" aria-label="Tema" class="temi">
      {#each app.temi as t (t.id)}
        <button type="button" role="radio" aria-checked={scelto?.id === t.id} onclick={() => scegliPreferenze({ tema: t.id })}>
          <span class="anteprima" aria-hidden="true">
            {#each VERSIONI as v (v)}
              <span class="meta" style:background={t[v].sfondo}>
                <span style:color={t[v].inchiostro} style:font-family={t.font?.titoli ? `'${t.font.titoli}', serif` : undefined}>Aa</span>
                <span class="segni"><i style:background={t[v].inchiostro}></i><i style:background={t[v].accento}></i></span>
              </span>
            {/each}
          </span>
          <span class="nome"><span class="serif">{t.nome}</span></span>
        </button>
      {/each}
    </div>
    <button type="button" class="pulsante tratteggiato" disabled><Icona nome="carica" size={18} />Carica un tema (.json)</button>
    <p class="nota">Un tema è un solo file con font e colori, chiari e scuri. Il caricamento da qui è in arrivo: per ora l'admin copia il file nella cartella dei temi del server, e chiunque può sceglierlo.</p>
  </section>

  {#if app.utente?.role !== 'admin'}
  <section class="notifiche">
    <h2 class="dato">Notifiche push · in arrivo</h2>
    {#each NOTIFICHE as [nome, descrizione] (nome)}
      <Interruttore {nome} {descrizione} attivo={false} disabilitato />
    {/each}
    <button type="button" class="pulsante prova" disabled>Invia una notifica di prova</button>
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
