<script lang="ts">
  import { t } from '../i18n';
  import { onMount } from 'svelte';
  import Avatar from '../ui/Avatar.svelte';
  import Foglio from '../ui/Foglio.svelte';
  import Icona from '../ui/Icona.svelte';
  import NuovoGruppo from '../ui/NuovoGruppo.svelte';
  import Moderazione from '../ui/Moderazione.svelte';
  import { statoBlocco } from '../moderazione';
  import { api, type InAttesa, type Invito, type Moderato, type Persona, type Ruolo } from '../api';
  import { app, avvisa, messaggio, sincronizza } from '../store.svelte';
  import { commuta, NOMI_RUOLO } from '../events';
  import { daSecondi, mese, quantoFa } from '../dates';
  import { orologio } from '../orologio.svelte';
  import { linkInvito, codiceLeggibile } from '../inviti';

  // L'admin gestisce persone, ruoli e ogni gruppo; il rappresentante inviti, richieste e gruppi della sua classe.
  const admin = $derived(app.utente?.role === 'admin');
  type Scheda = 'persone' | 'gruppi' | 'inviti';
  type Filtro = 'tutti' | Ruolo;
  let scheda = $state<Scheda>(app.utente?.role === 'admin' ? 'persone' : 'inviti');
  let filtro = $state<Filtro>('tutti');
  let cerca = $state('');
  let persone = $state<Persona[]>([]);
  let attesa = $state<InAttesa[]>([]);
  let inviti = $state<Invito[]>([]);
  let moderati = $state<Moderato[]>([]);
  let daModerare = $state<Moderato | null>(null);
  const stato = (id: string) => moderati.find((m) => m.id === id);
  let errore = $state('');

  async function carica() {
    try {
      const [p, i, m] = await Promise.all([api.persone(), api.inviti(), api.moderazione()]);
      persone = p.users; attesa = p.pending; inviti = i.invites; moderati = m.users; errore = '';
    } catch (e) { errore = messaggio(e); }
  }
  onMount(() => void carica());

  /** Esegue un'operazione di amministrazione e rilegge tutto. */
  async function esegui(op: () => Promise<unknown>, fatto?: string): Promise<boolean> {
    try {
      await op();
      await Promise.all([carica(), sincronizza()]);
      if (fatto) avvisa(fatto);
      return true;
    } catch (e) { avvisa(messaggio(e)); return false; }
  }

  const gruppi = $derived(app.meta?.groups.filter((g) => g.manage) ?? []);
  const nomeGruppo = (id: string | null) => app.meta?.groups.find((g) => g.id === id)?.name;
  const SCHEDE = $derived<[Scheda, string][]>([...(admin ? [['persone', t('Persone')] as [Scheda, string]] : []), ['gruppi', t('Gruppi')], ['inviti', t('Inviti')]]);
  const nomiGruppi = (ids: string[]) => gruppi.filter((g) => ids.includes(g.id)).map((g) => g.name).join(' · ') || t('Nessun gruppo');
  const conta = (r: Filtro) => (r === 'tutti' ? persone.length : persone.filter((p) => p.role === r).length);
  const FILTRI: [Filtro, string][] = [['tutti', t('Tutti')], ['admin', t('Admin')], ['rappresentante', t('Rappr.')], ['nerd', t('Nerd')], ['alunno', t('Alunni')]];
  const DESCRIZIONI: Record<Ruolo, string> = {
    admin: t('Admin: solo la parte tecnica. Utenti, gruppi, plugin, temi e backup. Non fa parte della classe e non vede il calendario.'),
    rappresentante: t('Rappresentanti: gestiscono eventi pubblici, materie e tipi e approvano le proposte dei Nerd. Non toccano la parte tecnica.'),
    nerd: t('Nerd: aiutano i rappresentanti. Le loro modifiche agli eventi pubblici vanno approvate.'),
    alunno: t('Alunni: aggiungono interrogazioni ed eventi personali. Non propongono modifiche.'),
  };
  const elenco = $derived(persone.filter((p) => (filtro === 'tutti' || p.role === filtro) && `${p.display_name} ${p.username}`.toLowerCase().includes(cerca.trim().toLowerCase())));

  // --- Modifica di una persona ---
  let scelta = $state<Persona | null>(null);
  let ruolo = $state<Ruolo>('alunno');
  let suoi = $state<string[]>([]);
  function apri(p: Persona) { scelta = p; ruolo = p.role; suoi = [...p.group_ids]; }
  /** Un gruppo di una classe porta con sé la classe; togliendo la classe si tolgono i suoi gruppi. */
  function spunta(id: string) {
    let nuovi = commuta(suoi, id);
    const g = gruppi.find((x) => x.id === id);
    if (nuovi.includes(id)) {
      if (g?.parent_id && !nuovi.includes(g.parent_id)) nuovi = [...nuovi, g.parent_id];
    } else nuovi = nuovi.filter((x) => gruppi.find((y) => y.id === x)?.parent_id !== id);
    suoi = nuovi;
  }
  const io = $derived(scelta?.id === app.utente?.id);
  async function salva() {
    const p = scelta!;
    if (await esegui(() => api.salvaPersona(p.id, { role: io ? undefined : ruolo, group_ids: ruolo === 'admin' ? undefined : $state.snapshot(suoi) }), t('Salvato.'))) scelta = null;
  }
  /** Un admin non compare nell'elenco della moderazione: si può comunque sospendere o riattivare. */
  function modera(p: Persona) {
    daModerare = stato(p.id) ?? { id: p.id, name: p.display_name, role: p.role, class_ids: [], photo: null, block: p.disabled ? { kind: 'sospeso', until: null, reason: '', tags: [] } : null, renaming: false, can_moderate: true };
    scelta = null;
  }

  // --- Eliminazione ---
  let daEliminare = $state<Persona | null>(null);
  let eliminando = $state(false);
  function chiediElimina(p: Persona) { daEliminare = p; scelta = null; }
  async function elimina() {
    const p = daEliminare!;
    eliminando = true;
    if (await esegui(() => api.eliminaPersona(p.id), t('Account di {nome} eliminato.', { nome: p.display_name }))) daEliminare = null;
    eliminando = false;
  }

  // --- Gruppi e inviti ---
  let nuovoGruppo = $state(false);
  // Membri di un gruppo interno a una classe (rappresentante).
  let gruppoScelto = $state<string | null>(null);
  let membri = $state<string[]>([]);
  const inModifica = $derived(gruppi.find((g) => g.id === gruppoScelto));
  const compagni = $derived(app.meta?.groups.find((g) => g.id === inModifica?.parent_id)?.members ?? []);
  function apriGruppo(g: (typeof gruppi)[number]) { gruppoScelto = g.id; membri = g.members.map((m) => m.id); }
  async function salvaGruppo() {
    if (await esegui(() => api.salvaMembri(gruppoScelto!, $state.snapshot(membri)), t('Salvato.'))) gruppoScelto = null;
  }
  const sottotitolo = (g: (typeof gruppi)[number]) => {
    const n = (r: Ruolo) => g.members.filter((m) => m.role === r).length;
    const r = n('rappresentante'), k = n('nerd');
    const tipo = g.kind === 'classe' ? t('Classe') : g.parent_id ? t('Gruppo di {nome}', { nome: nomeGruppo(g.parent_id) ?? t('classe') }) : t('Gruppo');
    return `${tipo} · ${r === 1 ? t('1 rappresentante') : t('{n} rappresentanti', { n: r })}, ${t('{n} nerd', { n: k })}`;
  };
  const scadenza = (s: number) => `${daSecondi(s).getDate()} ${mese(daSecondi(s))}`;
  async function copia(code: string) {
    const link = linkInvito(app.meta?.settings.public_url, code);
    try { await navigator.clipboard.writeText(link); avvisa(t('Link copiato.')); }
    catch { avvisa(link); }
  }
  function invita() { scheda = 'inviti'; }
</script>

<div class="schermata con-menu persone">
  <a href="#/profilo" class="torna"><Icona nome="sinistra" size={20} />{t('Profilo')}</a>
  <span class="dato occhio">{admin ? t('Pannello admin') : t('Rappresentante')}</span>
  <header>
    <h1>{admin ? t('Persone') : t('Classe')}</h1>
    <button type="button" class="pulsante pieno" onclick={invita}><Icona nome="piu" size={16} spessore={1.8} />{t('Invita')}</button>
  </header>

  <div role="tablist" aria-label={t('Sezioni')} class="schede">
    {#each SCHEDE as [id, nome] (id)}
      <button type="button" role="tab" aria-selected={scheda === id} onclick={() => (scheda = id)}>{nome}{#if id === 'inviti' && attesa.length}<span class="pallino">{attesa.length}</span>{/if}</button>
    {/each}
  </div>

  {#if errore}<p class="errore" role="alert">{errore}</p>{/if}

  {#if scheda === 'persone'}
    <label class="cerca">
      <Icona nome="cerca" size={18} />
      <span class="nascosto">{t('Cerca persone')}</span>
      <input type="search" bind:value={cerca} placeholder={t('Cerca per nome')} />
    </label>
    <div role="radiogroup" aria-label={t('Filtra per ruolo')} class="ruoli">
      {#each FILTRI as [id, nome] (id)}
        <button type="button" role="radio" class="scelta" aria-checked={filtro === id} onclick={() => (filtro = id)}>{nome} <span class="mono">{conta(id)}</span></button>
      {/each}
    </div>
    <p class="nota">{filtro === 'tutti' ? persone.length === 1 ? t('1 persona in tutto.') : t('{n} persone in tutto.', { n: persone.length }) : DESCRIZIONI[filtro]}</p>
    <ul class="elenco">
      {#each elenco as p (p.id)}
        <li class:disattivato={p.disabled}>
          <Avatar nome={p.display_name} persona={p.id} foto={stato(p.id)?.photo} tono={p.role === 'admin' ? 'pieno' : p.role === 'rappresentante' ? 'tenue' : 'neutro'} />
          <span class="voce chi">
            <span>{p.display_name}</span>
            <span class="tenue">{stato(p.id)?.block ? `${statoBlocco(stato(p.id)!.block!)} · ` : p.disabled ? t('Sospeso · ') : stato(p.id)?.renaming ? t('Deve cambiare nome · ') : ''}{p.role === 'admin' ? t('Gestione del server') : nomiGruppi(p.group_ids)}</span>
          </span>
          <button type="button" class="cambia" aria-label={t('Cambia ruolo e gruppi di {nome}', { nome: p.display_name })} onclick={() => apri(p)}>
            {NOMI_RUOLO[p.role].toUpperCase()}<Icona nome="giu" size={14} />
          </button>
        </li>
      {/each}
    </ul>
    {#if !elenco.length}<p class="nota">{t('Nessuno corrisponde.')}</p>{/if}

  {:else if scheda === 'gruppi'}
    <p class="lede">{admin ? t('Un evento pubblico arriva a tutti i membri del gruppo scelto.') : t('Puoi creare gruppi dentro la tua classe e scegliere chi ne fa parte.')}</p>
    <ul class="elenco">
      {#each gruppi as g (g.id)}
        <li class="gruppo">
          <span class="voce"><span class="serif">{g.name}</span><span class="nota">{sottotitolo(g)}</span></span>
          {#if !admin && g.parent_id}
            <button type="button" class="cambia" aria-label={t('Scegli i membri di {nome}', { nome: g.name })} onclick={() => apriGruppo(g)}>{t('MEMBRI {n}', { n: g.members.length })}<Icona nome="giu" size={14} /></button>
          {:else}
            <span class="mono">{g.members.length}</span>
          {/if}
        </li>
      {/each}
    </ul>
    <button type="button" class="pulsante tratteggiato" onclick={() => (nuovoGruppo = true)}>{t('+ Nuovo gruppo')}</button>
    {#if !admin}<a class="collegamento" href="#/lavori">{t('Gruppi di lavoro temporanei')}</a>{/if}
    {#if admin}<p class="nota">{t('Per aggiungere o togliere persone da un gruppo, apri il loro ruolo nella scheda Persone.')}</p>{/if}

  {:else}
    {#each gruppi as g (g.id)}
      {@const i = inviti.find((x) => x.group_id === g.id)}
      <div class="riquadro">
        <span class="dato">{i ? t('Codice attivo') : t('Nessun codice attivo')} · {g.name}</span>
        {#if i}
          <span class="codice">{codiceLeggibile(i.code)}</span>
          <span class="nota">{i.uses === 1 ? t('Usato 1 volta') : t('Usato {n} volte', { n: i.uses })} · {t('scade il {data}', { data: scadenza(i.expires_at) })}</span>
          <div class="due">
            <button type="button" class="pulsante" onclick={() => copia(i.code)}>{t('Copia link')}</button>
            <button type="button" class="pulsante" onclick={() => esegui(() => api.revocaInvito(i.id), t('Codice revocato.'))}>{t('Revoca')}</button>
          </div>
        {:else}
          <button type="button" class="pulsante genera" onclick={() => esegui(() => api.creaInvito(g.id))}>{t('Genera codice invito')}</button>
        {/if}
      </div>
    {:else}
      {#if admin}
        <p class="nota">{t('Crea prima una classe o un gruppo: il codice invito è legato a un gruppo.')}</p>
        <button type="button" class="pulsante tratteggiato" onclick={() => (nuovoGruppo = true)}>{t('+ Nuovo gruppo')}</button>
      {:else}
        <p class="nota">{t('Non fai ancora parte di una classe: chiedi all\'admin di aggiungerti.')}</p>
      {/if}
    {/each}

    <h2 class="dato attesa">{t('In attesa di conferma')}</h2>
    <ul class="elenco">
      {#each attesa as p (p.id)}
        <li>
          <span class="voce chi"><span>{p.display_name}</span><span class="tenue">{t('Richiesta {quando}', { quando: quantoFa(p.created_at, orologio.adesso) })} · {gruppi.find((g) => g.id === p.group_id)?.name ?? t('senza gruppo')}</span></span>
          <span class="azioni">
            <button type="button" class="tondo no" aria-label={t('Rifiuta {nome}', { nome: p.display_name })} onclick={() => esegui(() => api.rifiuta(p.id), t('Richiesta rifiutata.'))}><Icona nome="chiudi" size={16} spessore={1.6} /></button>
            <button type="button" class="tondo si" aria-label={t('Accetta {nome}', { nome: p.display_name })} onclick={() => esegui(() => api.accetta(p.id), t('{nome} è dentro.', { nome: p.display_name }))}><Icona nome="spunta" size={16} spessore={1.8} /></button>
          </span>
        </li>
      {/each}
    </ul>
    {#if !attesa.length}<p class="nota">{t('Nessuna richiesta.')}</p>{/if}
  {/if}
</div>

{#if scelta}
  <Foglio titolo={scelta.display_name} onclose={() => (scelta = null)}>
    <fieldset>
      <legend class="dato">{t('Ruolo')}</legend>
      <div class="scelte" role="radiogroup" aria-label={t('Ruolo')}>
        {#each Object.entries(NOMI_RUOLO) as [id, nome] (id)}
          <button type="button" role="radio" class="scelta" aria-checked={ruolo === id} disabled={io} onclick={() => (ruolo = id as Ruolo)}>{nome}</button>
        {/each}
      </div>
      <p class="nota spiega">{io ? t('Non puoi cambiare il tuo ruolo.') : DESCRIZIONI[ruolo]}</p>
    </fieldset>
    {#if ruolo !== 'admin'}
      <fieldset>
        <legend class="dato">{t('Gruppi')}</legend>
        {#each gruppi as g (g.id)}
          <label class="spunta"><input type="checkbox" checked={suoi.includes(g.id)} onchange={() => spunta(g.id)} /><span>{g.name}</span></label>
        {:else}
          <p class="nota">{t('Nessun gruppo ancora.')}</p>
        {/each}
      </fieldset>
    {:else if scelta.role !== 'admin'}
      <p class="nota">{t('Salvando, esce da tutti i gruppi e non vede più il calendario, nemmeno i propri eventi privati.')}</p>
    {/if}
    <button type="button" class="pulsante pieno grande" onclick={salva}>{t('Salva')}</button>
    {#if !io}
      <button type="button" class="pulsante" onclick={() => modera(scelta!)}>{t('Sospendi, espelli, nome o foto…')}</button>
      {#if admin}<button type="button" class="collegamento elimina" onclick={() => chiediElimina(scelta!)}>{t('Elimina account…')}</button>{/if}
    {/if}
  </Foglio>
{/if}

{#if daEliminare}
  <Foglio titolo={t('Eliminare l\'account?')} onclose={() => (daEliminare = null)}>
    <p>{t('{nome} non potrà più entrare. Non si può annullare.', { nome: daEliminare.display_name })}</p>
    <p class="nota">{t('Si cancellano accesso, foto, eventi privati ed eventi che riguardano solo questa persona. Gli eventi pubblicati per la classe restano, firmati «Account eliminato».')}</p>
    <button type="button" class="pulsante pieno grande" disabled={eliminando || !app.online} aria-busy={eliminando} onclick={elimina}>{t('Elimina per sempre')}</button>
    <button type="button" class="pulsante" onclick={() => (daEliminare = null)}>{t('Annulla')}</button>
  </Foglio>
{/if}

{#if daModerare}<Moderazione persona={daModerare} onclose={() => (daModerare = null)} onfatto={carica} />{/if}

{#if inModifica}
  <Foglio titolo={inModifica.name} onclose={() => (gruppoScelto = null)}>
    <fieldset>
      <legend class="dato">{t('Membri')}</legend>
      {#each compagni as m (m.id)}
        <label class="spunta"><input type="checkbox" checked={membri.includes(m.id)} onchange={() => (membri = commuta(membri, m.id))} /><span>{m.name}{m.id === app.utente?.id ? t(' (tu)') : ''}</span></label>
      {/each}
    </fieldset>
    <button type="button" class="pulsante pieno grande" onclick={salvaGruppo}>{t('Salva')}</button>
  </Foglio>
{/if}

{#if nuovoGruppo}
  <NuovoGruppo onclose={() => (nuovoGruppo = false)} onfatto={carica} />
{/if}

<style>
  .persone { gap: 16px; }
  .occhio { letter-spacing: .1em; }
  header { display: flex; align-items: flex-end; justify-content: space-between; gap: 12px; margin-top: -8px; }
  h1 { font-size: 58px; line-height: .9; }
  header .pulsante { min-height: 42px; border: 0; font-size: 14px; }
  .cerca { display: flex; align-items: center; gap: 8px; min-height: 44px; padding: 0 12px; border: 1px solid var(--filetto); background: var(--superficie); }
  .cerca :global(svg) { color: var(--attenuato); }
  .cerca input { flex: 1; min-width: 0; border: 0; background: transparent; font-size: 15px; outline-offset: 4px; }
  .ruoli { display: flex; flex-wrap: wrap; gap: 6px; }
  .ruoli .scelta { min-height: 34px; font-size: 13px; padding: 0 11px; }
  .ruoli .mono { font-size: 11px; opacity: .75; }
  .chi { flex: 1; font-size: 15px; }
  .chi .tenue { font-size: 12px; }
  .disattivato .chi { color: var(--attenuato); }
  .cambia { min-height: 44px; padding: 0 2px 0 8px; display: flex; align-items: center; gap: 4px; font-family: var(--font-dati); font-size: 10.5px; letter-spacing: .07em; }
  .elenco > li.gruppo { min-height: 76px; }
  .gruppo .serif { font-size: 30px; line-height: 1.05; }
  .gruppo .mono { font-size: 13px; }
  .codice { font-family: var(--font-dati); font-size: 24px; letter-spacing: .12em; }
  .due { display: flex; gap: 10px; padding-top: 8px; }
  .due .pulsante { flex: 1; min-height: 42px; font-size: 14px; }
  .genera { margin-top: 6px; }
  .attesa { padding-top: 6px; }
  .azioni { display: flex; gap: 6px; }
  .no { border: 1px solid var(--filetto); }
  .si { background: var(--inchiostro); color: var(--sfondo); }
  legend { padding: 0 0 10px; }
  .spiega { padding-top: 10px; }
  .elimina { align-self: center; color: var(--accento); }
  .spunta { display: flex; align-items: center; gap: 10px; min-height: 44px; border-bottom: 1px solid var(--filetto); }
</style>
