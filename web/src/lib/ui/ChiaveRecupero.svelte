<script lang="ts">
  let { chiave, onfatto, cta = "L'ho salvata, continua" }: { chiave: string; onfatto: () => void; cta?: string } = $props();
</script>

<div class="schermata">
  <span class="marchio">TaskHit</span>
  <span class="dato">Passo obbligatorio</span>
  <h1>Conserva la chiave di recupero</h1>
  <p class="lede">Se dimentichi la password, questa chiave è l'unico modo di riavere i tuoi dati privati. Il server non la conserva: scrivila o stampala e tienila al sicuro.</p>
  <p class="chiave" aria-label="Chiave di recupero">{chiave}</p>
  <button type="button" class="pulsante accento grande" onclick={onfatto}>{cta}</button>
</div>

<style>
  h1 { font-size: 44px; line-height: .98; text-wrap: balance; }
  .chiave { font-family: var(--font-dati); font-size: 18px; letter-spacing: .06em; line-height: 1.8; padding: 16px; background: var(--superficie); border: 1px dashed var(--inchiostro); overflow-wrap: anywhere; user-select: all; }
</style>
