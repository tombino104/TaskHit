<script lang="ts">
  // Riga con titolo, spiegazione e interruttore.
  let { nome, descrizione = '', attivo, disabilitato = false, onchange }: {
    nome: string; descrizione?: string; attivo: boolean; disabilitato?: boolean; onchange?: (v: boolean) => void;
  } = $props();
  const id = $props.id();
</script>

<div class="riga">
  <span class="voce">
    <span {id}>{nome}</span>
    {#if descrizione}<span class="nota">{descrizione}</span>{/if}
  </span>
  <button type="button" role="switch" aria-checked={attivo} aria-labelledby={id} disabled={disabilitato} onclick={() => onchange?.(!attivo)}>
    <i></i>
  </button>
</div>

<style>
  .riga { display: flex; align-items: center; justify-content: space-between; gap: 14px; min-height: 60px; padding: 8px 0; border-bottom: 1px solid var(--filetto); }
  button { flex: none; width: 50px; height: 30px; border-radius: 15px; border: 1.5px solid var(--inchiostro); padding: 2px; display: flex; justify-content: flex-start; }
  button i { display: block; width: 23px; height: 23px; border-radius: 50%; background: var(--inchiostro); }
  button[aria-checked='true'] { background: var(--inchiostro); justify-content: flex-end; }
  button[aria-checked='true'] i { background: var(--sfondo); }
</style>
