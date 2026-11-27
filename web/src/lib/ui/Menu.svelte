<script lang="ts">
  // Navigazione principale: barra in basso sul telefono.
  import Icona, { type NomeIcona } from './Icona.svelte';
  import { app } from '../store.svelte';
  import { rotta } from '../router.svelte';

  type Voce = { id: string; nome: string; icona: NomeIcona };
  const admin = $derived(app.utente?.role === 'admin');
  const gestione: Voce[] = [
    { id: 'persone', nome: 'Persone', icona: 'profilo' },
    { id: 'backup', nome: 'Backup', icona: 'aggiorna' },
    { id: 'profilo', nome: 'Profilo', icona: 'info' },
  ];
  const voci: Voce[] = [
    { id: 'oggi', nome: 'Oggi', icona: 'oggi' },
    { id: 'calendario', nome: 'Calendario', icona: 'calendario' },
    { id: 'inbox', nome: 'Inbox', icona: 'inbox' },
    { id: 'profilo', nome: 'Profilo', icona: 'profilo' },
  ];
</script>

{#snippet voce(v: Voce)}
  <a href="#/{v.id}" aria-current={rotta.nome === v.id ? 'page' : undefined}>
    <Icona nome={v.icona} spessore={rotta.nome === v.id ? 1.6 : 1.5} />{v.nome}
    {#if v.id === 'inbox' && app.inbox.unread && rotta.nome !== 'inbox'}
      <span class="conta"><span class="nascosto">Non lette: </span>{app.inbox.unread > 99 ? '99+' : app.inbox.unread}</span>
    {:else if v.id === 'persone' && app.meta?.pending && rotta.nome !== 'persone'}
      <span class="conta"><span class="nascosto">Richieste in attesa: </span>{app.meta.pending}</span>
    {:else if v.id === 'profilo' && !admin && app.meta?.pending && rotta.nome !== 'profilo'}
      <span class="conta"><span class="nascosto">Richieste in attesa: </span>{app.meta.pending}</span>
    {/if}
  </a>
{/snippet}

<nav aria-label="Navigazione principale" class:gestione={admin}>
  {#if admin}
    {#each gestione as v (v.id)}{@render voce(v)}{/each}
  {:else}
    {@render voce(voci[0]!)}
    {@render voce(voci[1]!)}
    <a href="#/nuovo" class="aggiungi" aria-label="Nuovo evento"><Icona nome="piu" spessore={1.8} /></a>
    {@render voce(voci[2]!)}
    {@render voce(voci[3]!)}
  {/if}
</nav>

<style>
  nav { position: fixed; z-index: 10; left: 0; right: 0; bottom: 0; padding: 6px 10px calc(6px + max(var(--basso), 6px)); background: var(--sfondo); border-top: 1px solid var(--filetto); display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); align-items: center; max-width: 620px; margin: 0 auto; }
  a { position: relative; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 2px; min-height: 48px; color: var(--attenuato); font-size: 11px; }
  nav.gestione { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  a[aria-current='page'] { color: var(--inchiostro); font-weight: 600; }
  .aggiungi { justify-self: center; width: 50px; height: 50px; border-radius: 50%; background: var(--inchiostro); color: var(--sfondo); }
  .conta { position: absolute; top: 2px; left: calc(50% + 6px); min-width: 16px; height: 16px; padding: 0 4px; border-radius: 8px; background: var(--accento); color: var(--su-accento); font-size: 10px; font-weight: 600; display: flex; align-items: center; justify-content: center; }
  @media (min-width: 900px) { nav { display: none; } }
</style>
