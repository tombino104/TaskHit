<script lang="ts">
  import { t } from '../i18n';
  import Avatar from '../ui/Avatar.svelte';
  import Icona from '../ui/Icona.svelte';
  import RigaEvento from '../ui/RigaEvento.svelte';
  import Sblocco from '../ui/Sblocco.svelte';
  import { app } from '../store.svelte';
  import { colore, filtra, fraseOggi, perGiorno } from '../events';
  import { chiave, giornoBreve, maiuscola, mese, nomeGiorno, numeroSettimana, settimana, dataLunga, daChiave } from '../dates';
  import { orologio } from '../orologio.svelte';

  const adesso = $derived(orologio.adesso);
  const oggi = $derived(chiave(adesso));
  const visibili = $derived(filtra(app.eventi, app.filtri));
  const giorni = $derived(perGiorno(visibili));
  const diOggi = $derived(giorni.get(oggi) ?? []);
  const prossimi = $derived([...giorni.keys()].filter((k) => k > oggi).sort().slice(0, 2));
  const classe = $derived(app.meta?.groups.find((g) => g.mine && g.kind === 'classe')?.name);
  const testata = $derived(
    [classe ?? app.meta?.settings.instance_name, app.meta?.settings.school_year && t('A.S. {anno}', { anno: app.meta.settings.school_year })].filter(Boolean).join(' · '),
  );
</script>

<div class="schermata con-menu oggi">
  <header>
    <div class="alto">
      <span class="marchio">TaskHit</span>
      <div class="azioni">
        <a href="#/cerca" class="tondo" aria-label={t('Cerca')}><Icona nome="cerca" /></a>
        <a href="#/profilo" class="profilo" aria-label={t('Il tuo profilo')}><Avatar nome={app.utente?.display_name ?? ''} persona={app.utente?.id} /></a>
      </div>
    </div>
    <div class="dato striscia">
      <span>{testata || t('Calendario')}{app.online ? '' : t(' · offline')}</span>
      <span>{t('Settimana {n}', { n: numeroSettimana(adesso) })}</span>
    </div>
  </header>

  <section class="apertura">
    <h1>{maiuscola(nomeGiorno(adesso))}</h1>
    <p class="data">{adesso.getDate()} {mese(adesso)}</p>
    <p class="frase">{fraseOggi(app.eventi, app.meta, adesso)}</p>
  </section>

  <ul class="settimana" aria-label={t('Settimana')}>
    {#each settimana(adesso) as d, i (i)}
      {@const k = chiave(d)}
      {@const del = giorni.get(k) ?? []}
      <li>
        <a href="#/calendario/{k}" class:presente={k === oggi} class:festivo={i >= 5} aria-current={k === oggi ? 'date' : undefined}
           aria-label="{dataLunga(d)}{del.length ? `, ${del.length === 1 ? t('1 evento') : t('{n} eventi', { n: del.length })}` : ''}">
          <span class="sigla">{giornoBreve(d).toUpperCase()}</span>
          <span class="numero">{d.getDate()}</span>
          <span class="punti">{#each del.slice(0, 3) as e (e.id)}<i style:background={colore(app.meta, e.tag_id)}></i>{/each}</span>
        </a>
      </li>
    {/each}
  </ul>

  <Sblocco />

  <section aria-label={t('Oggi')} class="lista">
    <h2 class="dato occhiello"><span>{t('Oggi')}</span><span>{diOggi.length}</span></h2>
    {#each diOggi as e (e.id)}
      <RigaEvento evento={e} />
    {:else}
      <p class="vuoto">{t('Giornata libera. Nessun evento.')}</p>
    {/each}
  </section>

  {#if !diOggi.length}
    {#each prossimi as k (k)}
      <section class="lista">
        <h2 class="dato occhiello"><span>{dataLunga(daChiave(k))}</span><span>{giorni.get(k)!.length}</span></h2>
        {#each giorni.get(k)! as e (e.id)}<RigaEvento evento={e} />{/each}
      </section>
    {/each}
  {/if}
</div>

<style>
  header { display: flex; flex-direction: column; gap: 10px; }
  .alto { display: flex; align-items: center; justify-content: space-between; }
  .azioni { display: flex; align-items: center; gap: 4px; }
  .profilo { display: flex; }
  .striscia { display: flex; justify-content: space-between; gap: 12px; border-top: 1px solid var(--inchiostro); border-bottom: 1px solid var(--filetto); padding: 6px 0; }
  .apertura { display: flex; flex-direction: column; gap: 6px; }
  h1 { font-size: clamp(62px, 20vw, 78px); line-height: .88; letter-spacing: -.015em; }
  .data { font-family: var(--font-titoli); font-style: italic; font-size: 38px; line-height: 1; color: var(--inchiostro-2); }
  .frase { margin-top: 8px; color: var(--inchiostro-2); text-wrap: pretty; }
  .settimana { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); border-top: 1px solid var(--filetto); border-bottom: 1px solid var(--filetto); padding: 10px 0; margin: 0; }
  .settimana a { display: flex; flex-direction: column; align-items: center; gap: 4px; min-height: 44px; }
  .sigla { font-family: var(--font-dati); font-size: 10px; color: var(--attenuato); }
  .numero { font-size: 17px; width: 32px; height: 32px; border-radius: 50%; display: flex; align-items: center; justify-content: center; }
  .punti { display: flex; gap: 3px; height: 6px; }
  .punti i { width: 6px; height: 6px; }
  .festivo { color: var(--attenuato); }
  .presente .sigla { color: var(--inchiostro); font-weight: 500; }
  .presente .numero { background: var(--inchiostro); color: var(--sfondo); font-weight: 600; }
  .lista { display: flex; flex-direction: column; }
  h2.occhiello { padding-bottom: 4px; }
  .vuoto { padding: 18px 0; font-size: 14px; color: var(--attenuato); }
  @media (min-width: 900px) { .alto { display: none; } }
</style>
