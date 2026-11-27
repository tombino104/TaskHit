<script lang="ts">
  import { t } from '../i18n';
  // Colonna laterale su schermi larghi: sezioni, materie visibili e utente.
  import Avatar from './Avatar.svelte';
  import { app } from '../store.svelte';
  import { rotta } from '../router.svelte';
  import { NOMI_RUOLO } from '../events';
  import { decideProposte, gruppiDiLavoro, vedeProposte } from '../lavori';

  const admin = $derived(app.utente?.role === 'admin');
  const sezioni = $derived((admin
    ? [
        { id: 'persone', nome: t('Persone'), pallino: app.meta?.pending },
        { id: 'backup', nome: t('Backup') },
        { id: 'plugin', nome: t('Plugin') },
        { id: 'profilo', nome: t('Profilo') },
        { id: 'aspetto', nome: t('Impostazioni') },
      ]
    : [
        { id: 'oggi', nome: t('Oggi') },
        { id: 'calendario', nome: t('Calendario') },
        { id: 'inbox', nome: t('Inbox'), pallino: app.inbox.unread },
        ...(app.utente && vedeProposte(app.utente, app.meta) ? [{ id: 'approvazioni', nome: decideProposte(app.utente, app.meta) ? t('Da approvare') : t('Le mie proposte'), pallino: app.inbox.to_approve }] : []),
        ...(app.meta?.groups.some((g) => g.mine && g.kind === 'classe') ? [{ id: 'classe', nome: t('La mia classe') }] : []),
        ...(app.utente?.role === 'rappresentante' || gruppiDiLavoro(app.meta).length ? [{ id: 'lavori', nome: t('Gruppi di lavoro') }] : []),
        ...(app.utente?.role === 'rappresentante' ? [{ id: 'persone', nome: t('Gestione classe'), pallino: app.meta?.pending }] : []),
        { id: 'profilo', nome: t('Profilo') },
        { id: 'aspetto', nome: t('Impostazioni') },
      ]
  ) as { id: string; nome: string; pallino?: number }[]);

  const tutte = $derived(app.meta?.tags.map((t) => t.id) ?? []);
  const visibile = (id: string) => !app.filtri.materie.length || app.filtri.materie.includes(id);
  /** Lista vuota = tutte visibili. Se si toglie l'ultima, tornano visibili tutte. */
  function commutaMateria(id: string) {
    const scelte = app.filtri.materie.length ? app.filtri.materie : tutte;
    const nuove = scelte.includes(id) ? scelte.filter((x) => x !== id) : [...scelte, id];
    app.filtri.materie = nuove.length === tutte.length ? [] : nuove;
  }
  const gruppo = $derived(app.meta?.groups.find((g) => g.mine && g.kind === 'classe')?.name);
</script>

<aside>
  <a href={admin ? '#/persone' : '#/oggi'} class="marchio">TaskHit</a>

  <nav aria-label={t('Sezioni')}>
    {#each sezioni as s (s.id)}
      <a href="#/{s.id}" aria-current={rotta.nome === s.id ? 'page' : undefined}>
        {s.nome}
        {#if s.pallino}<span class="pallino"><span class="nascosto">{t('Da vedere:')} </span>{s.pallino}</span>{/if}
      </a>
    {/each}
  </nav>

  {#if (rotta.nome === 'calendario' || rotta.nome === 'oggi') && app.meta}
    <fieldset>
      <legend class="dato">{t('Materie')}</legend>
      {#each app.meta.tags as t (t.id)}
        <label>
          <input type="checkbox" checked={visibile(t.id)} onchange={() => commutaMateria(t.id)} />
          <i class="quadratino grande" style:background={t.color}></i>{t.name}
        </label>
      {/each}
    </fieldset>
  {/if}

  {#if app.utente}
    <a href="#/profilo" class="utente">
      <Avatar nome={app.utente.display_name} persona={app.utente.id} />
      <span class="voce">
        <span class="nome">{app.utente.display_name}</span>
        <span class="tenue">{NOMI_RUOLO[app.utente.role]}{gruppo ? ` · ${gruppo}` : ''}</span>
      </span>
    </a>
  {/if}
</aside>

<style>
  aside { display: none; }
  @media (min-width: 900px) {
    aside { display: flex; flex-direction: column; gap: 28px; width: 270px; flex: none; height: 100dvh; position: sticky; top: 0; overflow-y: auto; padding: 28px 24px; border-right: 1px solid var(--filetto); }
  }
  .marchio { font-size: 34px; }
  nav { display: flex; flex-direction: column; }
  nav a { display: flex; align-items: center; justify-content: space-between; min-height: 40px; color: var(--inchiostro-2); border-bottom: 1px solid var(--filetto); }
  nav a[aria-current='page'] { color: var(--inchiostro); font-weight: 600; border-bottom-color: var(--inchiostro); }
  legend { padding: 0 0 8px; }
  label { display: flex; align-items: center; gap: 10px; min-height: 34px; font-size: 14px; cursor: pointer; }
  label input { width: 16px; height: 16px; }
  .utente { margin-top: auto; display: flex; align-items: center; gap: 10px; padding-top: 14px; border-top: 1px solid var(--filetto); }
  .nome { font-size: 14px; font-weight: 500; }
  .utente .tenue { font-size: 12px; }
</style>
