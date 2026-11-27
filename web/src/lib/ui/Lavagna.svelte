<script lang="ts">
  import { t } from '../i18n';
  // Composizione dei gruppi: si trascina un nome in un gruppo, oppure lo si tocca e poi si sceglie «Qui».
  // Il trascinamento usa i pointer event (quello nativo non funziona al tocco). Sul telefono si trascina
  // dalla maniglia, così il resto del nome lascia scorrere la pagina.
  import Icona from './Icona.svelte';
  import { daAssegnare, nominaCapo, sposta, type Bozza } from '../lavori';

  let { persone, bozze = $bindable(), fisso = false }: {
    persone: { id: string; name: string }[]; bozze: Bozza[];
    /** Gruppi già esistenti: non si rinominano né si tolgono. */
    fisso?: boolean;
  } = $props();

  const libere = $derived(daAssegnare(persone, bozze));
  const nome = (id: string) => persone.find((p) => p.id === id)?.name ?? t('Non più in classe');

  let scelta = $state<string | null>(null);
  let trascina = $state<{ id: string; x: number; y: number; attivo: boolean } | null>(null);
  let sopra = $state<string | null>(null);
  let partenza = { x: 0, y: 0 };
  let appenaTrascinato = false;

  function metti(id: string, zona: number | null) {
    bozze = sposta(bozze, id, zona);
    scelta = null;
  }

  function giu(e: PointerEvent, id: string) {
    appenaTrascinato = false;
    if (e.button !== 0 || (e.pointerType === 'touch' && !(e.target as Element).closest('.presa'))) return;
    partenza = { x: e.clientX, y: e.clientY };
    trascina = { id, x: e.clientX, y: e.clientY, attivo: false };
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
  }
  const zonaSotto = (x: number, y: number) => document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-zona]')?.dataset.zona ?? null;
  function muovi(e: PointerEvent) {
    if (!trascina) return;
    const attivo = trascina.attivo || Math.hypot(e.clientX - partenza.x, e.clientY - partenza.y) > 8;
    trascina = { ...trascina, x: e.clientX, y: e.clientY, attivo };
    if (!attivo) return;
    sopra = zonaSotto(e.clientX, e.clientY);
    // Vicino ai bordi la pagina scorre, per arrivare ai gruppi fuori vista.
    if (e.clientY < 80) scrollBy(0, -14);
    else if (e.clientY > innerHeight - 130) scrollBy(0, 14);
  }
  function su() {
    if (trascina?.attivo) {
      appenaTrascinato = true;
      const zona = zonaSotto(trascina.x, trascina.y);
      if (zona !== null) metti(trascina.id, zona === 'fuori' ? null : Number(zona));
    }
    annulla();
  }
  function annulla() { trascina = null; sopra = null; }
  function tocca(id: string) {
    if (appenaTrascinato) { appenaTrascinato = false; return; }
    scelta = scelta === id ? null : id;
  }
  const dentro = (zona: number | null) => (zona === null ? libere.some((p) => p.id === scelta) : bozze[zona]!.membri.includes(scelta!));
</script>

<svelte:window onkeydown={(e) => { if (e.key === 'Escape') { scelta = null; annulla(); } }} />

{#snippet persona(id: string)}
  <button type="button" class="nome" aria-pressed={scelta === id} class:via={trascina?.attivo && trascina.id === id}
    onpointerdown={(e) => giu(e, id)} onpointermove={muovi} onpointerup={su} onpointercancel={annulla} onclick={() => tocca(id)}>
    <span class="presa" aria-hidden="true"><i></i><i></i><i></i></span><span>{nome(id)}</span>
  </button>
{/snippet}

{#snippet qui(zona: number | null, dove: string)}
  {#if scelta && !dentro(zona)}
    <button type="button" class="qui" onclick={() => metti(scelta!, zona)}>{t('Metti {nome} {dove}', { nome: nome(scelta), dove })}</button>
  {/if}
{/snippet}

<p class="nota" role="status">{scelta ? t('{nome} è selezionato: scegli dove metterlo.', { nome: nome(scelta) }) : t('Trascina un nome in un gruppo, oppure toccalo e scegli dove metterlo. La stella indica il capogruppo.')}</p>

<section class="zona libere" data-zona="fuori" class:sopra={sopra === 'fuori'} aria-label={t('Da assegnare')}>
  <h2 class="dato">{t('Da assegnare · {n}', { n: libere.length })}</h2>
  <div class="nomi">
    {#each libere as p (p.id)}{@render persona(p.id)}{:else}<span class="nota">{t('Tutti hanno un gruppo.')}</span>{/each}
  </div>
  {@render qui(null, t('tra quelli da assegnare'))}
</section>

<div class="gruppi">
  {#each bozze as b, i (i)}
    <section class="zona" data-zona={i} class:sopra={sopra === String(i)} aria-label={b.nome}>
      <div class="testa">
        {#if fisso}<h2 class="serif">{b.nome}</h2>
        {:else}
          <input class="serif" bind:value={b.nome} maxlength="40" aria-label={t('Nome del gruppo {n}', { n: i + 1 })} />
          <button type="button" class="tondo" aria-label={t('Togli {nome}', { nome: b.nome })} onclick={() => (bozze = bozze.filter((_, k) => k !== i))}><Icona nome="chiudi" size={16} /></button>
        {/if}
        <span class="mono tenue">{b.membri.length}</span>
      </div>
      <ul>
        {#each b.membri as m (m)}
          <li>
            {@render persona(m)}
            <button type="button" class="stella" aria-pressed={b.capo === m} aria-label={t('{nome} capogruppo di {gruppo}', { nome: nome(m), gruppo: b.nome })} onclick={() => (bozze = nominaCapo(bozze, i, m))}>
              <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><path d="M12 3.5l2.6 5.4 5.9.8-4.3 4.1 1.1 5.9L12 16.9l-5.3 2.8 1.1-5.9-4.3-4.1 5.9-.8z" /></svg>
              {#if b.capo === m}<span>{t('Capo')}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
      {#if !b.membri.length}<p class="vuoto">{t('Trascina qui')}</p>{/if}
      {@render qui(i, t('in {gruppo}', { gruppo: b.nome }))}
    </section>
  {/each}
</div>

{#if trascina?.attivo}
  <div class="fantasma" style:left="{trascina.x}px" style:top="{trascina.y}px" aria-hidden="true">{nome(trascina.id)}</div>
{/if}

<style>
  .zona { display: flex; flex-direction: column; gap: 10px; padding: 12px; border: 1px solid var(--filetto); background: var(--superficie); min-width: 0; }
  .zona.libere { border-style: dashed; border-color: var(--attenuato); background: transparent; }
  .zona.sopra { border-color: var(--accento); outline: 2px solid var(--accento); background: var(--accento-tenue); }
  .nomi { display: flex; flex-wrap: wrap; gap: 8px; }
  .gruppi { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px; }
  .nome { display: inline-flex; align-items: center; min-height: 40px; max-width: 100%; padding: 0 12px 0 0; border: 1px solid var(--filetto); background: var(--sfondo); font-size: 14px; text-align: left; cursor: grab; user-select: none; -webkit-user-select: none; -webkit-touch-callout: none; }
  .nome[aria-pressed='true'] { border-color: var(--inchiostro); background: var(--inchiostro); color: var(--sfondo); }
  .nome.via { opacity: .35; }
  .presa { display: inline-flex; flex-direction: column; justify-content: center; align-items: center; gap: 3px; align-self: stretch; width: 34px; flex: none; touch-action: none; }
  .presa i { width: 12px; height: 1.5px; background: currentColor; opacity: .55; }
  .testa { display: flex; align-items: center; gap: 8px; border-bottom: 1px solid var(--inchiostro); padding-bottom: 4px; }
  .testa h2, .testa input { flex: 1; min-width: 0; font-size: 22px; line-height: 1.2; }
  .testa input { border: 0; background: transparent; padding: 4px 0; font-family: var(--font-titoli); }
  .testa .tondo { width: 36px; height: 36px; }
  .testa .mono { font-size: 12px; }
  ul { display: flex; flex-direction: column; gap: 6px; }
  li { display: flex; align-items: center; justify-content: space-between; gap: 6px; }
  .stella { display: inline-flex; align-items: center; gap: 4px; min-height: 40px; min-width: 40px; justify-content: center; font-size: 12px; color: var(--attenuato); flex: none; }
  .stella path { fill: none; stroke: currentColor; stroke-width: 1.5; stroke-linejoin: round; }
  .stella[aria-pressed='true'] { color: var(--accento); font-weight: 600; }
  .stella[aria-pressed='true'] path { fill: currentColor; }
  .vuoto { padding: 14px; text-align: center; font-size: 13px; color: var(--attenuato); border: 1px dashed var(--filetto); }
  .qui { min-height: 44px; border: 1px solid var(--accento); color: var(--accento); font-size: 13.5px; font-weight: 500; }
  .fantasma { position: fixed; z-index: 40; transform: translate(-50%, -120%); padding: 8px 14px; background: var(--inchiostro); color: var(--sfondo); font-size: 14px; pointer-events: none; white-space: nowrap; }
</style>
