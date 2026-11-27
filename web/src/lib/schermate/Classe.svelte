<script lang="ts">
  import { t } from '../i18n';
  // I compagni di classe, con foto e ruolo. I rappresentanti da qui moderano alunni e nerd.
  import Avatar from '../ui/Avatar.svelte';
  import Icona from '../ui/Icona.svelte';
  import Moderazione from '../ui/Moderazione.svelte';
  import { api, type Moderato } from '../api';
  import { app, messaggio } from '../store.svelte';
  import { NOMI_RUOLO } from '../events';
  import { statoBlocco } from '../moderazione';

  const rappresentante = $derived(app.utente?.role === 'rappresentante');
  const classi = $derived(app.meta?.groups.filter((g) => g.mine && g.kind === 'classe') ?? []);

  let moderati = $state<Moderato[]>([]);
  let errore = $state('');
  async function carica() {
    if (app.utente?.role !== 'rappresentante') return;
    try { moderati = (await api.moderazione()).users; errore = ''; } catch (e) { errore = messaggio(e); }
  }
  $effect(() => { void app.ultimaSync; void carica(); });

  let scelto = $state<Moderato | null>(null);
  const moderabile = (id: string) => moderati.find((p) => p.id === id && p.can_moderate);
  const sospesi = $derived(moderati.filter((p) => p.block));
</script>

<div class="schermata con-menu classe">
  <a href="#/profilo" class="torna"><Icona nome="sinistra" size={20} />{t('Profilo')}</a>
  <h1 class="titolo sottolineato">{t('La mia classe')}</h1>
  {#if errore}<p class="errore" role="alert">{errore}</p>{/if}

  {#each classi as c (c.id)}
    <section aria-label={c.name}>
      <h2 class="dato">{c.name} · {c.members.length === 1 ? t('1 persona') : t('{n} persone', { n: c.members.length })}</h2>
      <ul class="elenco">
        {#each c.members as m (m.id)}
          <li>
            <Avatar nome={m.name} persona={m.id} size={48} tono={m.role === 'rappresentante' ? 'tenue' : 'neutro'} />
            <span class="voce chi"><span>{m.name}{m.id === app.utente?.id ? t(' (tu)') : ''}</span><span class="tenue">{NOMI_RUOLO[m.role]}{moderabile(m.id)?.renaming ? t(' · deve cambiare nome') : ''}</span></span>
            {#if moderabile(m.id)}<button type="button" class="collegamento" aria-label={t('Modera {nome}', { nome: m.name })} onclick={() => (scelto = moderabile(m.id)!)}>{t('Modera')}</button>{/if}
          </li>
        {/each}
      </ul>
    </section>
  {:else}
    <p class="lede">{t('Non fai ancora parte di una classe.')}</p>
  {/each}

  {#if rappresentante && sospesi.length}
    <section aria-label={t('Sospesi')}>
      <h2 class="dato">{t('Sospesi')}</h2>
      <ul class="elenco">
        {#each sospesi as p (p.id)}
          <li>
            <Avatar nome={p.name} persona={p.id} foto={p.photo} size={48} tono="neutro" />
            <span class="voce chi"><span>{p.name}</span><span class="tenue">{statoBlocco(p.block!)}</span></span>
            {#if p.can_moderate}<button type="button" class="collegamento" aria-label={t('Modera {nome}', { nome: p.name })} onclick={() => (scelto = p)}>{t('Modera')}</button>
            {:else}<span class="nota">{t('Deciso dall’admin')}</span>{/if}
          </li>
        {/each}
      </ul>
    </section>
  {/if}
</div>

{#if scelto}<Moderazione persona={scelto} onclose={() => (scelto = null)} onfatto={carica} />{/if}

<style>
  .classe { gap: 20px; }
  .titolo { margin-top: -8px; font-size: clamp(42px, 12vw, 54px); }
  section { display: flex; flex-direction: column; gap: 8px; }
  .elenco > li { min-height: 68px; }
  .chi { flex: 1; font-size: 15px; }
  .chi .tenue { font-size: 12px; }
</style>
