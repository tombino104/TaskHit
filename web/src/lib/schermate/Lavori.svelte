<script lang="ts">
  import { t } from '../i18n';
  // I gruppi di lavoro visibili: i propri e, per i rappresentanti, quelli della classe.
  import Foglio from '../ui/Foglio.svelte';
  import Icona from '../ui/Icona.svelte';
  import { api, type Gruppo } from '../api';
  import { app, avvisa, messaggio, sincronizza } from '../store.svelte';
  import { orologio } from '../orologio.svelte';
  import { vai } from '../router.svelte';
  import { commuta } from '../events';
  import { daSecondi, mese } from '../dates';
  import { giorniRimasti, gruppiDiLavoro, nomeBreve, prorogaPossibile } from '../lavori';

  const rappresentante = $derived(app.utente?.role === 'rappresentante');
  const adesso = $derived(orologio.adesso.getTime() / 1000);
  const tutti = $derived(gruppiDiLavoro(app.meta));
  /** Raggruppati per lavoro: prima quelli aperti. */
  const lavori = $derived.by(() => {
    const out = new Map<string, { nome: string; chiuso: boolean; gruppi: Gruppo[] }>();
    for (const g of tutti) {
      const chiuso = g.work!.closed;
      const k = `${g.parent_id}|${g.work!.activity}|${chiuso}`;
      const l = out.get(k) ?? { nome: g.work!.activity, chiuso, gruppi: [] };
      l.gruppi.push(g); out.set(k, l);
    }
    return [...out.entries()].sort((a, b) => Number(a[1].chiuso) - Number(b[1].chiuso));
  });
  const data = (s: number) => `${daSecondi(s).getDate()} ${mese(daSecondi(s))}`;
  function tempo(g: Gruppo): string {
    const w = g.work!;
    if (w.closed) return t('Chiuso · si elimina il {data}', { data: data(w.deletes_at) });
    const n = giorniRimasti(w.expires_at, adesso);
    return n <= 1 ? t('Scade il {data}', { data: data(w.expires_at) }) : t('Ancora {n} giorni · fino al {data}', { n, data: data(w.expires_at) });
  }

  // --- Gestione di un gruppo ---
  let scelto = $state<string | null>(null);
  let membri = $state<string[]>([]);
  let capo = $state('');
  let conferma = $state<'' | 'chiudi' | 'elimina'>('');
  let occupato = $state(false);
  const gruppo = $derived(tutti.find((g) => g.id === scelto && g.work!.can_manage && !g.work!.closed));
  const compagni = $derived(app.meta?.groups.find((g) => g.id === gruppo?.parent_id)?.members ?? []);
  const proroga = $derived(gruppo ? prorogaPossibile(gruppo.work!) : 0);

  function apri(g: Gruppo) { scelto = g.id; membri = g.members.map((m) => m.id); capo = g.work!.leader_id ?? ''; conferma = ''; }
  function cambia(id: string) { membri = commuta(membri, id); if (!membri.includes(capo)) capo = ''; }

  async function esegui(op: () => Promise<unknown>, fatto: string, chiudi = true) {
    occupato = true;
    try {
      await op(); await sincronizza(); avvisa(fatto);
      if (chiudi) scelto = null;
    } catch (e) { avvisa(messaggio(e)); } finally { occupato = false; }
  }
</script>

<div class="schermata con-menu lavori">
  <a href="#/profilo" class="torna"><Icona nome="sinistra" size={20} />{t('Profilo')}</a>
  <header>
    <h1 class="titolo">{t('Gruppi di lavoro')}</h1>
    {#if rappresentante}<button type="button" class="pulsante pieno grande" onclick={() => vai('lavori/nuovo')}><Icona nome="piu" size={16} spessore={1.8} />{t('Componi i gruppi')}</button>{/if}
  </header>

  {#each lavori as [k, l] (k)}
    <section>
      <div class="testa">
        <h2 class="dato">{l.nome}{l.chiuso ? t(' · chiuso') : ''}</h2>
        {#if !l.chiuso && l.gruppi[0]!.work!.representative && l.gruppi.length > 1}
          <button type="button" class="collegamento" onclick={() => vai(`lavori/${l.gruppi[0]!.id}`)}>{t('Sposta le persone')}</button>
        {/if}
      </div>
      {#each l.gruppi as g (g.id)}
        <article class:chiuso={g.work!.closed}>
          <div class="riga">
            <span class="serif">{nomeBreve(g)}</span>
            {#if g.work!.can_manage && !g.work!.closed}<button type="button" class="collegamento" aria-label={t('Gestisci {nome}', { nome: g.name })} onclick={() => apri(g)}>{t('Gestisci')}</button>{/if}
          </div>
          <ul>
            {#each g.members as m (m.id)}
              <li class:capo={m.id === g.work!.leader_id}>{m.name}{m.id === app.utente?.id ? t(' (tu)') : ''}{#if m.id === g.work!.leader_id}<span class="dato">{t('Capogruppo')}</span>{/if}</li>
            {/each}
          </ul>
          {#if !g.work!.leader_id && !g.work!.closed}<p class="errore">{t('Manca il capogruppo: sceglilo in Gestisci.')}</p>{/if}
          <span class="nota">{tempo(g)}</span>
        </article>
      {/each}
    </section>
  {:else}
    <p class="lede">{rappresentante ? t('Ancora nessun gruppo. Componili per il prossimo lavoro.') : t('Non fai parte di nessun gruppo di lavoro. Li compongono i rappresentanti.')}</p>
  {/each}

  <p class="nota">{t('Gli eventi di un gruppo li vedono solo i suoi membri: il capogruppo li pubblica, gli altri glieli propongono. Alla scadenza il gruppo resta in sola lettura per 30 giorni.')}</p>
</div>

{#if gruppo}
  <Foglio titolo={nomeBreve(gruppo)} onclose={() => (scelto = null)}>
    <fieldset>
      <legend class="dato">{t('Membri e capogruppo')}</legend>
      {#each compagni as m (m.id)}
        <div class="membro">
          <label><input type="checkbox" checked={membri.includes(m.id)} onchange={() => cambia(m.id)} /><span>{m.name}{m.id === app.utente?.id ? t(' (tu)') : ''}</span></label>
          {#if membri.includes(m.id)}<label class="radio"><input type="radio" name="capo" checked={capo === m.id} onchange={() => (capo = m.id)} />{t('Capo')}</label>{/if}
        </div>
      {/each}
    </fieldset>
    <button type="button" class="pulsante pieno grande" disabled={occupato || !capo || !app.online} onclick={() => esegui(() => api.salvaGruppoLavoro(gruppo!.id, $state.snapshot(membri), capo), t('Salvato.'))}>
      {capo ? t('Salva') : t('Scegli il capogruppo')}
    </button>

    <fieldset>
      <legend class="dato">{t('Durata')}</legend>
      <p class="nota">{tempo(gruppo)}</p>
      <div class="scelte">
        {#each [...new Set([7, 14, proroga].filter((n) => n > 0 && n <= proroga))] as n (n)}
          <button type="button" class="scelta" disabled={occupato} onclick={() => esegui(() => api.prorogaGruppo(gruppo!.id, n), t('Prorogato.'), false)}>+ {n === 1 ? t('1 giorno') : t('{n} giorni', { n })}</button>
        {/each}
      </div>
      {#if !proroga}<p class="nota">{t('Ha già la durata massima di 90 giorni.')}</p>{/if}
    </fieldset>

    {#if conferma}
      <p class="errore">{conferma === 'chiudi' ? t('Diventa in sola lettura subito e si elimina tra 30 giorni. Non si riapre.') : t('Gruppo ed eventi si cancellano subito e non si recuperano.')}</p>
      <button type="button" class="pulsante accento grande" disabled={occupato} onclick={() => esegui(() => (conferma === 'chiudi' ? api.chiudiGruppo(gruppo!.id) : api.eliminaGruppo(gruppo!.id)), conferma === 'chiudi' ? t('Gruppo chiuso.') : t('Gruppo eliminato.'))}>
        {conferma === 'chiudi' ? t('Sì, chiudilo') : t('Sì, eliminalo')}
      </button>
      <button type="button" class="collegamento" onclick={() => (conferma = '')}>{t('Annulla')}</button>
    {:else}
      <button type="button" class="pulsante" onclick={() => (conferma = 'chiudi')}>{t('Chiudi ora il gruppo')}</button>
      {#if gruppo.work!.representative}<button type="button" class="collegamento" onclick={() => (conferma = 'elimina')}>{t('Elimina il gruppo')}</button>{/if}
    {/if}
  </Foglio>
{/if}

<style>
  .lavori { gap: 20px; }
  header { display: flex; flex-direction: column; gap: 16px; margin-top: -8px; }
  .titolo { font-size: clamp(42px, 12vw, 54px); }
  section { display: flex; flex-direction: column; gap: 10px; }
  .testa { display: flex; align-items: center; justify-content: space-between; gap: 8px; min-height: 32px; border-bottom: 1px solid var(--inchiostro); }
  article { display: flex; flex-direction: column; gap: 6px; padding: 12px 14px; background: var(--superficie); border: 1px solid var(--filetto); }
  article.chiuso { background: transparent; }
  .riga { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .riga .serif { font-size: 26px; line-height: 1.1; overflow-wrap: anywhere; }
  li { display: flex; align-items: baseline; gap: 8px; font-size: 14px; padding: 2px 0; }
  li.capo { font-weight: 600; }
  li .dato { color: var(--accento); }
  legend { padding: 0 0 8px; }
  .membro { display: flex; align-items: center; justify-content: space-between; gap: 12px; border-bottom: 1px solid var(--filetto); }
  .membro label { display: flex; align-items: center; gap: 10px; min-height: 44px; }
  .membro label:first-child { flex: 1; }
  .radio { font-size: 13px; }
  .radio input { width: 18px; height: 18px; margin: 0; accent-color: var(--accento); }
</style>
