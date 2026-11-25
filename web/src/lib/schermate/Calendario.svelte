<script lang="ts">
  import Icona from '../ui/Icona.svelte';
  import RigaEvento from '../ui/RigaEvento.svelte';
  import Sblocco from '../ui/Sblocco.svelte';
  import Grafico from '../ui/Grafico.svelte';
  import Filtri from './Filtri.svelte';
  import { app } from '../store.svelte';
  import { orologio } from '../orologio.svelte';
  import { largo } from '../schermo.svelte';
  import { rotta } from '../router.svelte';
  import { colore, filtra, filtriAttivi, perGiorno, riassunto } from '../events';
  import {
    chiave, chiaveValida, daChiave, dataLunga, giornoBreve, grigliaMese, maiuscola, mese, numeroSettimana,
    ora, piuGiorni, piuMesi, settimana,
  } from '../dates';
  import { stato } from './vistaCalendario.svelte';

  // Un giorno nell'indirizzo (#/calendario/2026-11-26) diventa quello selezionato.
  $effect(() => { if (chiaveValida(rotta.arg)) stato.giorno = rotta.arg; });

  const vista = $derived(stato.vista ?? (largo.attivo ? 'settimana' : 'mese'));
  const oggi = $derived(chiave(orologio.adesso));
  const scelto = $derived(daChiave(stato.giorno ?? oggi));
  const giorni = $derived(perGiorno(filtra(app.eventi, app.filtri)));
  const sett = $derived(settimana(scelto));
  const periodo = $derived(
    vista === 'mese' ? grigliaMese(scelto).filter((d) => d.getMonth() === scelto.getMonth()) : vista === 'settimana' ? sett : [scelto],
  );
  const nelPeriodo = (eventi: typeof app.eventi) => {
    const chiavi = new Set(periodo.map(chiave));
    return [...perGiorno(eventi)].filter(([k]) => chiavi.has(k)).reduce((n, [, l]) => n + l.length, 0);
  };
  const delGiorno = $derived(giorni.get(chiave(scelto)) ?? []);
  const intervallo = $derived.by(() => {
    const [a, b] = [sett[0]!, sett[6]!];
    return a.getMonth() === b.getMonth() ? { numeri: `${a.getDate()} — ${b.getDate()}`, mese: mese(a) }
      : { numeri: `${a.getDate()} ${mese(a).slice(0, 3)} — ${b.getDate()}`, mese: mese(b).slice(0, 3) };
  });
  const classe = $derived(app.meta?.groups.find((g) => g.mine && g.kind === 'classe')?.name);
  const graficoAttivo = $derived(!!app.meta?.plugins.grafico.user && !!app.meta.plugins.grafico.instance);
  const colonne = $derived(sett.map((d) => {
    const l = giorni.get(chiave(d)) ?? [];
    return { etichetta: giornoBreve(d), nome: dataLunga(d), privati: l.filter((e) => e.privato).length, pubblici: l.filter((e) => !e.privato).length };
  }));

  function scorri(verso: number) {
    const d = vista === 'mese' ? piuMesi(scelto, verso) : piuGiorni(scelto, verso * (vista === 'settimana' ? 7 : 1));
    stato.giorno = chiave(d);
  }
  const VISTE = [['giorno', 'Giorno'], ['settimana', 'Settimana'], ['mese', 'Mese']] as const;
  const unita = $derived(vista === 'mese' ? ['Mese precedente', 'Mese successivo'] : vista === 'settimana' ? ['Settimana precedente', 'Settimana successiva'] : ['Giorno precedente', 'Giorno successivo']);
</script>

<div class="schermata con-menu calendario" class:ampia={vista === 'settimana'}>
  <div class="cima">
  <header class:sett={vista === 'settimana'}>
    <div class="intestazione">
      {#if vista === 'settimana'}
        <span class="dato occhio">Settimana {numeroSettimana(scelto)}{classe ? ` · Classe ${classe}` : ''}</span>
        <h1 class="settimanale">{intervallo.numeri} <em>{intervallo.mese}</em></h1>
      {:else}
        <h1>{maiuscola(mese(scelto))}</h1>
        <span class="mono anno">{scelto.getFullYear()}</span>
      {/if}
    </div>
    <div class="frecce">
      <button type="button" class="tondo" aria-label={unita[0]} onclick={() => scorri(-1)}><Icona nome="sinistra" size={20} /></button>
      <button type="button" class="adesso" onclick={() => (stato.giorno = oggi)}>Oggi</button>
      <button type="button" class="tondo" aria-label={unita[1]} onclick={() => scorri(1)}><Icona nome="destra" size={20} /></button>
    </div>
  </header>

  <div class="barra">
    <div role="group" aria-label="Vista" class="segmenti viste">
      {#each VISTE as [id, nome] (id)}
        <button type="button" aria-pressed={vista === id} onclick={() => (stato.vista = id)}>{nome}</button>
      {/each}
    </div>
    <button type="button" class="filtra" onclick={() => (stato.filtriAperti = true)}>
      <Icona nome="filtro" size={18} />Filtra{#if filtriAttivi(app.filtri)}<span class="pallino">{filtriAttivi(app.filtri)}</span>{/if}
    </button>
    <a href="#/nuovo/{chiave(scelto)}" class="pulsante pieno nuovo"><Icona nome="piu" size={16} spessore={1.8} />Nuovo evento</a>
  </div>
  </div>

  <Sblocco />

  {#if vista === 'mese'}
    <div class="mese">
      <div class="sigle" aria-hidden="true">{#each sett as d, i (i)}<span>{giornoBreve(d).toUpperCase()}</span>{/each}</div>
      <div class="griglia">
        {#each grigliaMese(scelto) as d (chiave(d))}
          {@const k = chiave(d)}
          {@const fuori = d.getMonth() !== scelto.getMonth()}
          {@const l = giorni.get(k) ?? []}
          <button type="button" class:fuori class:presente={k === oggi} aria-pressed={k === chiave(scelto)}
                  aria-label="{dataLunga(d)}{l.length ? `, ${l.length} ${l.length === 1 ? 'evento' : 'eventi'}` : ''}"
                  onclick={() => (stato.giorno = k)}>
            <span class="numero">{d.getDate()}</span>
            <span class="barre">{#each l.slice(0, 3) as e (e.id)}<i style:background={colore(app.meta, e.tag_id)}></i>{/each}</span>
          </button>
        {/each}
      </div>
    </div>
  {/if}

  {#if vista !== 'settimana'}
    <section aria-live="polite" class="giornata">
      <div class="titolo-giorno">
        <h2>{dataLunga(scelto)}</h2>
        <span class="dato">{delGiorno.length} {delGiorno.length === 1 ? 'evento' : 'eventi'}</span>
      </div>
      {#each delGiorno as e (e.id)}
        <RigaEvento evento={e} compatta={vista === 'mese'} />
      {:else}
        <p class="vuoto">Giornata libera. Nessun evento.</p>
      {/each}
    </section>
  {:else if largo.attivo}
    <div class="scorrimento">
      <div class="colonne">
        {#each sett as d, i (chiave(d))}
          {@const k = chiave(d)}
          {@const l = giorni.get(k) ?? []}
          <section aria-label={dataLunga(d)} class:presente={k === oggi} class:festivo={i >= 5}>
            <h2><span class="dato">{giornoBreve(d)}</span><span class="grande">{d.getDate()}</span></h2>
            <div class="schede-evento">
              {#each l as e (e.id)}
                <a href="#/evento/{e.id}" style:border-top-color={colore(app.meta, e.tag_id)}>
                  <span class="mono riga"><span>{ora(e.starts_at)}</span><span>{e.privato ? 'PRIVATO' : 'PUBBLICO'}</span></span>
                  <span class="dato">{riassunto(app.meta, e)}</span>
                  <span class="nome">{e.title}</span>
                </a>
              {:else}
                <span class="libero">Libero</span>
              {/each}
            </div>
          </section>
        {/each}
      </div>
    </div>

    {#if graficoAttivo}
      <section class="carico" aria-label="Plugin: carico della settimana">
        <div class="legenda">
          <span class="dato">Plugin · Carico della settimana</span>
          <span class="chiavi"><span><i class="quadratino grande pub"></i>Pubblici</span><span><i class="quadratino grande priv"></i>Privati</span></span>
        </div>
        <Grafico {colonne} periodo="questa settimana" altezza={84} assi={false} />
      </section>
    {/if}
  {:else}
    {#each sett as d (chiave(d))}
      {@const l = giorni.get(chiave(d)) ?? []}
      <section class="giornata">
        <h2 class="dato occhiello"><span>{dataLunga(d)}</span><span>{l.length}</span></h2>
        {#each l as e (e.id)}<RigaEvento evento={e} compatta />{:else}<p class="vuoto stretto">Libero</p>{/each}
      </section>
    {/each}
  {/if}
</div>

{#if stato.filtriAperti}
  <Filtri onclose={() => (stato.filtriAperti = false)} conta={(f) => nelPeriodo(filtra(app.eventi, f))} />
{/if}

<style>
  .calendario { --margine: 20px; gap: 14px; }
  .cima { display: flex; flex-direction: column; gap: 14px; }
  header { display: flex; align-items: flex-end; justify-content: space-between; gap: 8px; padding: 0 4px; }
  header.sett { flex-wrap: wrap; }
  .intestazione { display: flex; flex-wrap: wrap; align-items: baseline; column-gap: 10px; }
  h1 { font-size: clamp(46px, 15vw, 60px); line-height: .9; letter-spacing: -.01em; }
  .occhio { width: 100%; }
  h1.settimanale { font-size: clamp(38px, 11vw, 64px); line-height: .95; }
  h1 em { color: var(--inchiostro-2); }
  .anno { font-size: 13px; color: var(--inchiostro-2); }
  .frecce { display: flex; align-items: center; }
  .adesso { display: none; }
  .barra { display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 0 4px; }
  .viste > * { flex: none; min-height: 34px; font-size: 13px; }
  .viste > [aria-pressed='false'] { color: var(--inchiostro-2); }
  .filtra { display: flex; align-items: center; gap: 6px; min-height: 44px; padding: 0 4px; font-size: 13px; }
  .nuovo { display: none; }
  .mese { display: flex; flex-direction: column; }
  .sigle { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); font-family: var(--font-dati); font-size: 10px; letter-spacing: .06em; color: var(--attenuato); border-bottom: 1px solid var(--inchiostro); padding-bottom: 6px; text-align: center; }
  .griglia { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); }
  .griglia button { height: 52px; border-bottom: 1px solid var(--filetto); padding: 5px 4px 4px; display: flex; flex-direction: column; align-items: center; gap: 4px; }
  .griglia button[aria-pressed='true'] { background: var(--superficie); outline: 1px solid var(--inchiostro); outline-offset: -1px; }
  .griglia button:focus-visible { outline: 2px solid var(--accento); outline-offset: -2px; }
  .numero { font-size: 14px; line-height: 1; width: 24px; height: 24px; border-radius: 50%; display: flex; align-items: center; justify-content: center; }
  button[aria-pressed='true'] .numero, .griglia .presente .numero { font-weight: 600; }
  .griglia .presente .numero { background: var(--inchiostro); color: var(--sfondo); }
  .fuori .numero { color: var(--attenuato); }
  .barre { display: flex; flex-direction: column; gap: 2px; width: 70%; }
  .barre i { display: block; height: 3px; }
  .giornata { display: flex; flex-direction: column; padding: 4px 4px 0; }
  .titolo-giorno { display: flex; align-items: baseline; justify-content: space-between; gap: 10px; padding-bottom: 6px; border-bottom: 1px solid var(--inchiostro); }
  .titolo-giorno h2 { font-style: italic; font-size: 28px; line-height: 1.1; }
  .vuoto { padding: 18px 0; font-size: 14px; color: var(--attenuato); }
  .vuoto.stretto { padding: 8px 0; }

  @media (min-width: 900px) {
    .calendario { max-width: 720px; }
    .calendario.ampia { max-width: none; --margine: 36px; gap: 22px; }
    header, .barra, .giornata { padding-left: 0; padding-right: 0; }
    .cima { flex-direction: row; flex-wrap: wrap; align-items: flex-end; justify-content: space-between; gap: 16px 24px; border-bottom: 1px solid var(--inchiostro); padding-bottom: 16px; }
    .frecce { gap: 10px; }
    .frecce .tondo { width: 40px; height: 40px; border: 1px solid var(--filetto); }
    .adesso { display: block; height: 40px; padding: 0 14px; border: 1px solid var(--filetto); font-size: 13.5px; }
    /* Titolo a sinistra, comandi in fila a destra. */
    header { display: contents; }
    .intestazione { margin-right: auto; }
    .barra { flex-wrap: wrap; justify-content: flex-start; }
    .viste > * { min-height: 38px; padding: 0 14px; font-size: 13.5px; }
    .nuovo { display: inline-flex; min-height: 40px; margin-left: 6px; font-size: 14px; }
  }

  .scorrimento { overflow-x: auto; }
  .colonne { min-width: 920px; display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); border-left: 1px solid var(--filetto); }
  .colonne section { border-right: 1px solid var(--filetto); border-top: 3px solid transparent; min-height: 470px; display: flex; flex-direction: column; }
  .colonne section.presente { border-top-color: var(--inchiostro); background: var(--superficie); }
  .colonne h2 { padding: 12px 14px 10px; display: flex; align-items: baseline; gap: 8px; border-bottom: 1px solid var(--filetto); }
  .colonne h2 .dato { font-size: 11px; color: var(--attenuato); }
  .presente h2 .dato { color: var(--inchiostro); }
  .grande { font-size: 40px; line-height: 1; }
  .festivo .grande { color: var(--attenuato); }
  .schede-evento { padding: 10px; display: flex; flex-direction: column; gap: 10px; }
  .schede-evento a { display: flex; flex-direction: column; gap: 5px; padding: 10px 10px 12px; background: var(--superficie); border: 1px solid var(--filetto); border-top-width: 3px; }
  .riga { display: flex; justify-content: space-between; gap: 4px; font-size: 11px; color: var(--inchiostro-2); }
  .schede-evento .dato { font-size: 10px; letter-spacing: .06em; }
  .nome { font-family: var(--font-titoli); font-size: 22px; line-height: 1.05; overflow-wrap: anywhere; }
  .libero { padding: 6px 4px; font-size: 13px; color: var(--attenuato); }
  .carico { display: flex; flex-direction: column; gap: 10px; }
  .legenda { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 8px; }
  .chiavi { display: flex; gap: 16px; font-size: 12.5px; color: var(--inchiostro-2); }
  .chiavi > span { display: flex; align-items: center; gap: 6px; }
  .pub { background: var(--accento); }
  .priv { background: var(--attenuato); }
</style>
