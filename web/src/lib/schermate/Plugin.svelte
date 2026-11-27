<script lang="ts">
  import Grafico from '../ui/Grafico.svelte';
  import Icona from '../ui/Icona.svelte';
  import Interruttore from '../ui/Interruttore.svelte';
  import Sblocco from '../ui/Sblocco.svelte';
  import { api } from '../api';
  import { app, avvisa, messaggio, sincronizza } from '../store.svelte';
  import { orologio } from '../orologio.svelte';
  import { perGiorno, puoGestire } from '../events';
  import { chiave, dataLunga, giornoBreve, grigliaMese, mese, meseBreve, piuGiorni, settimana } from '../dates';

  type Periodo = 'prossimi' | 'settimana' | 'mese';
  let periodo = $state<Periodo>('settimana');
  let pubblici = $state(true);
  let privati = $state(true);
  const PERIODI: [Periodo, string][] = [['prossimi', 'Prossimi 7 gg'], ['settimana', 'Settimana'], ['mese', 'Mese']];

  const adesso = $derived(orologio.adesso);
  const date = $derived(
    periodo === 'prossimi' ? Array.from({ length: 7 }, (_, i) => piuGiorni(adesso, i + 1))
    : periodo === 'settimana' ? settimana(adesso)
    : grigliaMese(adesso).filter((d) => d.getMonth() === adesso.getMonth()),
  );
  const intestazione = $derived.by(() => {
    if (periodo === 'mese') return `${mese(adesso)} ${adesso.getFullYear()}`;
    const [a, b] = [date[0]!, date.at(-1)!];
    return a.getMonth() === b.getMonth() ? `${a.getDate()} – ${b.getDate()} ${meseBreve(b)}` : `${a.getDate()} ${meseBreve(a)} – ${b.getDate()} ${meseBreve(b)}`;
  });
  const giorni = $derived(perGiorno(app.eventi));
  const colonne = $derived(date.map((d, i) => {
    const l = giorni.get(chiave(d)) ?? [];
    return {
      etichetta: periodo === 'mese' ? (i === 0 || d.getDate() % 5 === 0 ? String(d.getDate()) : '') : giornoBreve(d).charAt(0).toUpperCase(),
      nome: dataLunga(d).toLowerCase(),
      pubblici: pubblici ? l.filter((e) => !e.privato).length : 0,
      privati: privati ? l.filter((e) => e.privato).length : 0,
      oggi: chiave(d) === chiave(adesso),
    };
  }));
  const titolo = $derived.by(() => {
    const totali = colonne.map((c) => c.pubblici + c.privati);
    const somma = totali.reduce((a, b) => a + b, 0);
    if (!somma) return 'Niente in programma. Respira.';
    const pieno = colonne[totali.indexOf(Math.max(...totali))]!;
    return `${somma} ${somma === 1 ? 'evento' : 'eventi'}; il giorno più pieno è ${pieno.nome}.`;
  });

  const stato = $derived(app.meta?.plugins.grafico ?? { user: true, instance: true });
  const admin = $derived(app.utente?.role === 'admin');
  const decide = $derived(!!app.utente && puoGestire(app.utente.role));
  async function cambia(ambito: 'user' | 'instance', attivo: boolean) {
    try { await api.salvaPlugin('grafico', ambito, attivo); await sincronizza(); }
    catch (e) { avvisa(messaggio(e)); }
  }
</script>

<div class="schermata plugin">
  <a href="#/profilo" class="torna"><Icona nome="sinistra" size={20} />Profilo</a>
  <h1 class="titolo sottolineato">Plugin</h1>

  {#if admin}
    <p class="nota">Qui decidi quali plugin sono disponibili alla classe.</p>
  {:else if stato.user && stato.instance}
    <Sblocco />
    <section aria-label="Grafico della settimana" class="carta">
      <div class="testa"><span class="dato">Carico di lavoro</span><span class="dato tenue">{intestazione}</span></div>
      <p class="frase">{titolo}</p>
      <div role="radiogroup" aria-label="Periodo" class="segmenti">
        {#each PERIODI as [id, nome] (id)}
          <button type="button" role="radio" aria-checked={periodo === id} onclick={() => (periodo = id)}>{nome}</button>
        {/each}
      </div>
      <Grafico {colonne} periodo={intestazione} strette={periodo === 'mese'} />
      <div role="group" aria-label="Mostra" class="scelte">
        <button type="button" class="scelta contorno" aria-pressed={pubblici} onclick={() => (pubblici = !pubblici)}><i class="quadratino grande pub"></i>Pubblici</button>
        <button type="button" class="scelta contorno" aria-pressed={privati} onclick={() => (privati = !privati)}><i class="quadratino grande priv"></i>Privati</button>
      </div>
    </section>
  {:else}
    <p class="nota">{stato.instance ? 'Hai nascosto il grafico.' : 'Il grafico è disattivato per tutta la classe.'}</p>
  {/if}

  <section>
    <h2 class="dato">Installati</h2>
    <div class="voce-plugin">
      <span class="riga"><span class="serif">Grafico della settimana</span><span class="mono tenue">v1.0</span></span>
      <span>Quanti eventi hai giorno per giorno, pubblici e privati.</span>
    </div>
    {#if !admin}
      <Interruttore nome="Mostralo a me" descrizione="Puoi nasconderlo solo per te" attivo={stato.user} disabilitato={!app.online} onchange={(v) => cambia('user', v)} />
    {/if}
    <Interruttore nome="Attivo per tutta la classe" descrizione="Lo decidono admin e rappresentanti" attivo={stato.instance} disabilitato={!decide || !app.online} onchange={(v) => cambia('instance', v)} />
    <button type="button" class="pulsante tratteggiato installa" disabled><Icona nome="piu" size={18} />Installa plugin · in arrivo</button>
  </section>
</div>

<style>
  .plugin { gap: 20px; }
  .titolo { margin-top: -8px; }
  .carta { display: flex; flex-direction: column; gap: 14px; padding: 18px 16px 16px; background: var(--superficie); border: 1px solid var(--filetto); }
  .testa { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; }
  .frase { margin-top: -6px; font-family: var(--font-titoli); font-size: 30px; line-height: 1.05; text-wrap: balance; }
  .segmenti > * { min-height: 36px; font-size: 12.5px; padding: 0 4px; }
  .scelta { min-height: 34px; font-size: 13px; }
  .scelta[aria-pressed='false'] { border-color: var(--filetto); color: var(--attenuato); }
  .pub { background: var(--accento); }
  .priv { background: var(--attenuato); }
  section h2 { margin-bottom: 4px; }
  .voce-plugin { display: flex; flex-direction: column; gap: 2px; padding: 12px 0; border-bottom: 1px solid var(--filetto); font-size: 13px; color: var(--inchiostro-2); }
  .riga { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; color: var(--inchiostro); }
  .voce-plugin .serif { font-size: 26px; line-height: 1.1; }
  .voce-plugin .mono { font-size: 10.5px; }
  .installa { width: 100%; margin-top: 16px; }
</style>
