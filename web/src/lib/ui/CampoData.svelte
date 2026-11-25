<script lang="ts">
  // Campo data/ora con il testo scritto come nel resto dell'app; il selettore è quello nativo del dispositivo.
  let { etichetta, tipo, valore = $bindable(), testo, mono = false }: {
    etichetta: string; tipo: 'date' | 'time'; valore: string; testo: string; mono?: boolean;
  } = $props();

  function apri(ev: MouseEvent) {
    try { (ev.currentTarget as HTMLInputElement).showPicker(); } catch { /* il browser apre il suo selettore */ }
  }
</script>

<label>
  <span class="dato">{etichetta}</span>
  <span class="valore" class:mono aria-hidden="true">{testo}</span>
  <input type={tipo} bind:value={valore} onclick={apri} />
</label>

<style>
  label { position: relative; display: flex; flex-direction: column; gap: 2px; border-bottom: 1px solid var(--inchiostro); padding: 0 0 8px; min-height: 52px; cursor: pointer; }
  label:focus-within { outline: 2px solid var(--accento); outline-offset: 2px; }
  .valore { font-size: 16px; white-space: nowrap; }
  input { position: absolute; inset: 0; width: 100%; height: 100%; opacity: 0; cursor: pointer; }
</style>
