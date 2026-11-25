<script lang="ts">
  // Colonne impilate: eventi pubblici (accento) e privati (attenuato) per giorno.
  export interface Colonna { etichetta: string; nome: string; pubblici: number; privati: number; oggi?: boolean }
  let { colonne, periodo, altezza = 180, assi = true, strette = false }: {
    colonne: Colonna[]; periodo: string; altezza?: number; assi?: boolean; strette?: boolean;
  } = $props();

  const massimo = $derived(Math.max(3, ...colonne.map((c) => c.pubblici + c.privati)));
  const unita = $derived(altezza / massimo);
  /** Al massimo sei linee, anche con molti eventi. */
  const tacche = $derived.by(() => {
    const passo = Math.max(1, Math.ceil(massimo / 5));
    return Array.from({ length: Math.floor(massimo / passo) + 1 }, (_, i) => i * passo);
  });
  const descrizione = $derived(
    `Eventi per giorno, ${periodo.toLowerCase()}: ${colonne.filter((c) => c.pubblici + c.privati).map((c) => `${c.nome} ${c.pubblici + c.privati}`).join(', ') || 'nessuno'}`,
  );
</script>

<div role="img" aria-label={descrizione} class="grafico" class:assi class:strette>
  {#if assi}
    <div class="scala" aria-hidden="true" style:height="{altezza}px">
      {#each tacche as t (t)}<span style:bottom="{t * unita}px">{t}</span>{/each}
    </div>
  {/if}
  <div class="area">
    <div class="piano" style:height="{altezza}px">
      {#if assi}{#each tacche as t (t)}<i class="linea" aria-hidden="true" style:bottom="{t * unita}px"></i>{/each}{/if}
      <div class="colonne">
        {#each colonne as c, i (i)}
          <div class="colonna">
            <div class="pila">
              <i style:height="{c.pubblici * unita}px" class="pub"></i>
              <i style:height="{c.privati * unita}px" class="priv"></i>
            </div>
          </div>
        {/each}
      </div>
    </div>
    {#if assi}
      <div class="giorni" aria-hidden="true">
        {#each colonne as c, i (i)}<span class:oggi={c.oggi}>{c.etichetta}</span>{/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .grafico { --spazio: 14px; display: grid; grid-template-columns: minmax(0, 1fr); gap: 6px; }
  .grafico.assi { grid-template-columns: 18px minmax(0, 1fr); }
  .grafico.strette { --spazio: 2px; }
  .scala { position: relative; font-family: var(--font-dati); font-size: 10px; color: var(--attenuato); }
  .scala span { position: absolute; right: 0; transform: translateY(50%); line-height: 1; }
  .area { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .piano { position: relative; border-bottom: 1px solid var(--inchiostro); }
  .linea { position: absolute; left: 0; right: 0; height: 1px; background: var(--filetto); }
  .colonne { position: absolute; inset: 0; display: flex; align-items: flex-end; gap: var(--spazio); }
  .colonna { flex: 1; min-width: 0; height: 100%; display: flex; flex-direction: column; justify-content: flex-end; align-items: center; }
  .pila { width: 100%; display: flex; flex-direction: column-reverse; gap: 1px; }
  .grafico:not(.assi) .pila { width: 36%; gap: 2px; }
  .grafico:not(.assi) { --spazio: 0px; }
  .pila i { display: block; }
  .pub { background: var(--accento); }
  .priv { background: var(--attenuato); }
  .giorni { display: flex; gap: var(--spazio); font-family: var(--font-dati); font-size: 10px; color: var(--inchiostro-2); }
  .giorni span { flex: 1; min-width: 0; text-align: center; white-space: nowrap; }
  .oggi { font-weight: 600; }
</style>
