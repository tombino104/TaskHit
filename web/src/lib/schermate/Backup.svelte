<script lang="ts">
  // I backup automatici sono in roadmap: la pagina mostra come saranno, con i comandi disattivati.
  import { onMount } from 'svelte';
  import Icona from '../ui/Icona.svelte';
  import { api } from '../api';

  let cartella = $state('');
  onMount(() => { api.statoAdmin().then((s) => (cartella = s.backups_dir)).catch(() => { /* offline */ }); });
</script>

<div class="schermata backup">
  <a href="#/profilo" class="torna"><Icona nome="sinistra" size={20} />Profilo</a>
  <span class="dato occhio">Pannello admin</span>
  <h1 class="titolo sottolineato">Backup</h1>

  <section>
    <p class="stato">Backup automatici in arrivo.</p>
    <p class="nota">Questa versione non fa ancora copie da sola. Intanto salva tu l'intera cartella dei dati di TaskHit, segreti compresi.</p>
    <button type="button" class="pulsante" disabled><Icona nome="aggiorna" size={18} />Esegui un backup adesso</button>
  </section>

  <section>
    <h2 class="dato">Ricorrenza</h2>
    <div role="radiogroup" aria-label="Ricorrenza" class="segmenti">
      <button type="button" role="radio" aria-checked="true" disabled>Ogni giorno</button>
      <button type="button" role="radio" aria-checked="false" disabled>Ogni settimana</button>
      <button type="button" role="radio" aria-checked="false" disabled>Personalizzata</button>
    </div>
    <div class="quando">
      <button type="button" disabled><span class="dato">Giorni</span><span>Tutti</span></button>
      <button type="button" disabled><span class="dato">Orario</span><span class="mono">03:00</span></button>
    </div>
  </section>

  <section>
    <h2 class="dato">Conserva</h2>
    <div class="conserva">
      <span><span class="quanti">14</span><span>backup più recenti</span></span>
      <span class="passo">
        <button type="button" class="tondo" aria-label="Conserva meno backup" disabled><Icona nome="meno" size={18} spessore={1.6} /></button>
        <button type="button" class="tondo" aria-label="Conserva più backup" disabled><Icona nome="piu" size={18} spessore={1.6} /></button>
      </span>
    </div>
    <p class="nota">Quando ne arriverà uno nuovo, il più vecchio oltre il limite sarà eliminato in automatico.</p>
  </section>

  <section>
    <h2 class="dato">Archivio</h2>
    <p class="vuoto">Nessun backup.</p>
  </section>

  {#if cartella}<p class="destinazione">Destinazione prevista<br />{cartella}</p>{/if}
</div>

<style>
  .backup { gap: 20px; }
  .occhio { letter-spacing: .1em; }
  .titolo { margin-top: -12px; }
  section { display: flex; flex-direction: column; gap: 10px; }
  .stato { font-family: var(--font-titoli); font-size: 30px; line-height: 1.08; }
  .segmenti > * { min-height: 42px; }
  .segmenti > :disabled, .passo { opacity: .45; }
  .passo button { opacity: 1; }
  .quando { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
  .quando button { display: flex; flex-direction: column; align-items: flex-start; gap: 2px; border-bottom: 1px solid var(--inchiostro); padding-bottom: 8px; text-align: left; font-size: 16px; }
  .conserva { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .conserva > span:first-child { display: flex; align-items: baseline; gap: 8px; color: var(--inchiostro-2); }
  .quanti { font-family: var(--font-titoli); font-size: 56px; line-height: .9; color: var(--inchiostro); }
  .passo { display: flex; border: 1px solid var(--inchiostro); }
  .passo button { width: 46px; height: 46px; }
  .passo button + button { border-left: 1px solid var(--inchiostro); }
  .vuoto { border-top: 1px solid var(--inchiostro); padding: 16px 0; font-size: 14px; color: var(--attenuato); }
  .destinazione { font-family: var(--font-dati); font-size: 11px; color: var(--attenuato); line-height: 1.6; overflow-wrap: anywhere; }
</style>
