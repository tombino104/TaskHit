<script lang="ts">
  // Colonna laterale su schermi larghi: sezioni, materie visibili e utente.
  import Avatar from './Avatar.svelte';
  import { app } from '../store.svelte';
  import { rotta } from '../router.svelte';
  import { NOMI_RUOLO, puoPubblicare } from '../events';

  const admin = $derived(app.utente?.role === 'admin');
  const sezioni = $derived((admin
    ? [
        { id: 'persone', nome: 'Persone' },
        { id: 'backup', nome: 'Backup' },
        { id: 'plugin', nome: 'Plugin' },
        { id: 'profilo', nome: 'Profilo' },
        { id: 'aspetto', nome: 'Impostazioni' },
      ]
    : [
        { id: 'oggi', nome: 'Oggi' },
        { id: 'calendario', nome: 'Calendario' },
        { id: 'inbox', nome: 'Inbox', pallino: app.inbox.unread },
        ...(app.utente && puoPubblicare(app.utente.role) !== 'no' ? [{ id: 'approvazioni', nome: app.utente.role === 'nerd' ? 'Le mie proposte' : 'Da approvare', numero: app.inbox.to_approve }] : []),
        { id: 'profilo', nome: 'Profilo' },
        { id: 'aspetto', nome: 'Impostazioni' },
      ]
  ) as { id: string; nome: string; pallino?: number; numero?: number }[]);

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

  <nav aria-label="Sezioni">
    {#each sezioni as s (s.id)}
      <a href="#/{s.id}" aria-current={rotta.nome === s.id ? 'page' : undefined}>
        {s.nome}
        {#if s.pallino}<span class="pallino">{s.pallino}</span>
        {:else if s.numero}<span class="mono">{s.numero}</span>{/if}
      </a>
    {/each}
  </nav>

  {#if (rotta.nome === 'calendario' || rotta.nome === 'oggi') && app.meta}
    <fieldset>
      <legend class="dato">Materie</legend>
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
      <Avatar nome={app.utente.display_name} />
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
  nav .mono { font-size: 12px; font-weight: 400; }
  legend { padding: 0 0 8px; }
  label { display: flex; align-items: center; gap: 10px; min-height: 34px; font-size: 14px; cursor: pointer; }
  label input { width: 16px; height: 16px; }
  .utente { margin-top: auto; display: flex; align-items: center; gap: 10px; padding-top: 14px; border-top: 1px solid var(--filetto); }
  .nome { font-size: 14px; font-weight: 500; }
  .utente .tenue { font-size: 12px; }
</style>
