<script lang="ts">
  import Icona from '../ui/Icona.svelte';
  import Interruttore from '../ui/Interruttore.svelte';
  import { app, caricaTemi, scegliPreferenze } from '../store.svelte';
  import { pickTheme, type Modo } from '../theme';
  import type { Theme } from '../api';
  import { onMount } from 'svelte';

  onMount(() => void caricaTemi()); // un file aggiunto nella cartella compare subito

  const MODI: [Modo, string][] = [['chiaro', 'Chiaro'], ['scuro', 'Scuro'], ['sistema', 'Sistema']];
  const effettivo = (scuro: boolean) => pickTheme(app.temi, { ...app.preferenze, modo: 'sistema' }, scuro);
  const nota = $derived.by(() => {
    if (app.preferenze.modo === 'sistema') return `Segue il dispositivo: ${effettivo(false)?.nome} di giorno, ${effettivo(true)?.nome} di sera.`;
    return `Sempre ${pickTheme(app.temi, app.preferenze, false)?.nome}, qualunque sia l’impostazione del dispositivo.`;
  });
  const scelto = (t: Theme) => effettivo(t.tipo === 'scuro')?.id === t.id;
  /** Scegliere un tema lo assegna al giorno o alla sera; fuori da «Sistema» passa anche a quella modalità. */
  const scegli = (t: Theme) => scegliPreferenze({ [t.tipo]: t.id, modo: app.preferenze.modo === 'sistema' ? 'sistema' : t.tipo });
  const ordinati = $derived([...app.temi].sort((a, b) => a.tipo.localeCompare(b.tipo) || Number(!scelto(a)) - Number(!scelto(b))));

  const NOTIFICHE = [
    ['Nuovi eventi pubblici', 'Quando qualcuno aggiunge un evento per la classe'],
    ['Modifiche ed eliminazioni', 'Date, orari o eventi annullati'],
    ['Promemoria', 'Gli avvisi che imposti su ogni evento'],
    ['Proposte da approvare', 'Solo per rappresentanti e admin'],
    ['Non disturbare', 'Silenzia tutto dalle 22:00 alle 07:00'],
  ] as const;
</script>

<div class="schermata aspetto">
  <a href="#/profilo" class="torna"><Icona nome="sinistra" size={20} />Profilo</a>
  <h1 class="titolo sottolineato">Aspetto <em>&amp;</em> notifiche</h1>

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
      {#each ordinati as t (t.id)}
        <button type="button" role="radio" aria-checked={scelto(t)} onclick={() => scegli(t)}>
          <span class="anteprima" aria-hidden="true" style:background={t.colori.sfondo}>
            <span style:color={t.colori.inchiostro} style:font-family={t.font?.titoli ? `'${t.font.titoli}', serif` : undefined}>Aa</span>
            <span class="segni"><i style:background={t.colori.inchiostro}></i><i style:background={t.colori.accento}></i></span>
          </span>
          <span class="nome"><span class="serif">{t.nome}</span><span class="dato tenue">{t.tipo}</span></span>
        </button>
      {/each}
    </div>
    <button type="button" class="pulsante tratteggiato" disabled><Icona nome="carica" size={18} />Carica un tema (.json)</button>
    <p class="nota">Un tema è un solo file con colori e font. Il caricamento da qui è in arrivo: per ora l'admin copia il file nella cartella dei temi del server, e chiunque può sceglierlo.</p>
  </section>

  <section class="notifiche">
    <h2 class="dato">Notifiche push · in arrivo</h2>
    {#each NOTIFICHE as [nome, descrizione] (nome)}
      <Interruttore {nome} {descrizione} attivo={false} disabilitato />
    {/each}
    <button type="button" class="pulsante prova" disabled>Invia una notifica di prova</button>
  </section>
</div>

<style>
  .aspetto { gap: 22px; }
  .titolo { margin-top: -8px; font-size: 50px; line-height: .95; }
  section { display: flex; flex-direction: column; gap: 12px; }
  .segmenti > * { min-height: 44px; font-size: 14px; }
  .temi { display: grid; grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); gap: 12px; }
  .temi button { display: flex; flex-direction: column; gap: 8px; padding: 8px 8px 10px; border: 1px solid var(--filetto); text-align: left; }
  .temi [aria-checked='true'] { border: 1.5px solid var(--inchiostro); }
  .anteprima { display: flex; flex-direction: column; justify-content: space-between; height: 72px; padding: 10px; border: 1px solid rgba(127, 127, 127, .25); font-family: var(--font-titoli); font-size: 22px; line-height: 1; }
  .segni { display: flex; align-items: center; gap: 6px; }
  .segni i:first-child { height: 3px; width: 46%; }
  .segni i:last-child { width: 10px; height: 10px; border-radius: 50%; }
  .nome { display: flex; align-items: baseline; justify-content: space-between; gap: 6px; }
  .nome .serif { font-size: 22px; line-height: 1; overflow-wrap: anywhere; }
  .nome .dato { font-size: 9.5px; letter-spacing: .06em; }
  .notifiche { gap: 0; }
  .notifiche h2 { margin-bottom: 4px; }
  .prova { margin-top: 16px; font-size: 14px; }
</style>
