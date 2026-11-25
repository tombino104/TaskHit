<script lang="ts">
  // Prima configurazione in cinque passi. Il primo crea l'admin; gli altri si possono rivedere in seguito.
  import ChiaveRecupero from '../ui/ChiaveRecupero.svelte';
  import Icona from '../ui/Icona.svelte';
  import Interruttore from '../ui/Interruttore.svelte';
  import NuovoGruppo from '../ui/NuovoGruppo.svelte';
  import Foglio from '../ui/Foglio.svelte';
  import { api, type Invito, type Kdf, type StatoAdmin } from '../api';
  import { app, avvisa, messaggio, preparaChiavi, sincronizza, usaChiave } from '../store.svelte';
  import { codiceLeggibile, linkInvito } from '../inviti';

  let { kdf, onfine }: { kdf: Kdf | null; onfine: () => void } = $props();

  let passo = $state(1);
  let chiave = $state('');
  let errore = $state('');
  let occupato = $state(false);

  // Passo 1
  let nome = $state('');
  let username = $state('');
  let password = $state('');
  let password2 = $state('');
  // Passo 2
  const anno = new Date().getMonth() >= 7 ? new Date().getFullYear() : new Date().getFullYear() - 1;
  let istanza = $state('TaskHit');
  let indirizzo = $state(location.protocol === 'https:' ? location.origin : '');
  let annoScolastico = $state(`${anno}/${String(anno + 1).slice(2)}`);
  let fuso = $state(Intl.DateTimeFormat().resolvedOptions().timeZone);
  // Passi 3-5
  let inviti = $state<Invito[]>([]);
  let stato = $state<StatoAdmin | null>(null);
  let nuovoGruppo = $state(false);
  let nuovo = $state<'materia' | 'tipo' | ''>('');
  let nomeNuovo = $state('');
  let colore = $state('#2A9D8F');

  const PASSI = [
    ['Crea l’account admin', 'L’admin gestisce la parte tecnica: utenti, gruppi, plugin e backup.'],
    ['Dai un nome alla tua istanza', 'Niente è scritto nel codice: tutto quello che imposti qui si può cambiare dopo.'],
    ['Classi e gruppi', 'Solo l’admin può creare gruppi. Gli eventi pubblici arrivano a chi fa parte del gruppo.'],
    ['Materie e tag', 'Un colore per ogni materia, un’etichetta per ogni tipo di evento.'],
    ['Sicurezza e backup', 'Lo stato reale di questo server. Le impostazioni di sicurezza stanno in config.toml.'],
  ] as const;

  async function creaAdmin() {
    if (password.length < 10) throw new Error('La password deve avere almeno 10 caratteri.');
    if (password !== password2) throw new Error('Le password non coincidono.');
    const k = await preparaChiavi(password, kdf ?? undefined);
    const res = await api.setup({ username, display_name: nome || username, ...k.account });
    await usaChiave(k.dek, { username: res.user.username, kdf_salt: k.kdf_salt, kdf: kdf!, wrapped_key: k.account.wrapped_key });
    password = password2 = '';
    chiave = k.recupero;
    await sincronizza();
  }

  async function caricaInviti() {
    inviti = (await api.inviti()).invites;
    // Ogni gruppo ha subito il suo codice.
    const senza = app.meta?.groups.filter((g) => !inviti.some((i) => i.group_id === g.id)) ?? [];
    if (senza.length) {
      for (const g of senza) await api.creaInvito(g.id);
      inviti = (await api.inviti()).invites;
    }
  }

  async function avanti(ev: SubmitEvent) {
    ev.preventDefault(); errore = ''; occupato = true;
    try {
      if (passo === 1) { await creaAdmin(); passo = 2; }
      else if (passo === 2) {
        await api.salvaImpostazioni({ instance_name: istanza, public_url: indirizzo.trim(), school_year: annoScolastico, timezone: fuso });
        await sincronizza(); await caricaInviti(); passo = 3;
      } else if (passo === 3) passo = 4;
      else if (passo === 4) { stato = await api.statoAdmin(); passo = 5; }
      else onfine();
    } catch (e) { errore = messaggio(e); } finally { occupato = false; }
  }

  async function copia(code: string) {
    const link = linkInvito(indirizzo.trim(), code);
    try { await navigator.clipboard.writeText(link); avvisa('Link copiato.'); } catch { avvisa(link); }
  }

  async function aggiungi(ev: SubmitEvent) {
    ev.preventDefault();
    try {
      if (nuovo === 'materia') await api.creaMateria(nomeNuovo, colore); else await api.creaTipo(nomeNuovo);
      await sincronizza(); nomeNuovo = ''; nuovo = '';
    } catch (e) { avvisa(messaggio(e)); }
  }
</script>

{#if chiave}
  <ChiaveRecupero {chiave} onfatto={() => (chiave = '')} />
{:else}
  <div class="pagina">
    <div class="schermata setup">
      <div class="testa">
        <span class="marchio">TaskHit</span>
        <span class="dato">Prima configurazione</span>
      </div>

      <div class="avanzamento">
        <div aria-hidden="true">{#each [1, 2, 3, 4, 5] as i (i)}<i class:fatto={i <= passo}></i>{/each}</div>
        <span class="dato">Passo {passo} di 5</span>
      </div>

      <header>
        <h1>{PASSI[passo - 1]![0]}</h1>
        <p class="lede">{PASSI[passo - 1]![1]}</p>
      </header>

      <form id="passo" onsubmit={avanti}>
        {#if passo === 1}
          <label class="campo"><span class="dato">Nome</span><input bind:value={nome} maxlength="60" autocomplete="name" /></label>
          <label class="campo"><span class="dato">Nome utente</span><input class="mono" bind:value={username} required minlength="3" maxlength="32" autocomplete="username" autocapitalize="none" spellcheck="false" /></label>
          <label class="campo"><span class="dato">Password</span><input type="password" bind:value={password} required autocomplete="new-password" placeholder="Almeno 10 caratteri" />
            <span class="nota">Non lascia mai questo dispositivo. Nemmeno chi gestisce il server può leggerla.</span></label>
          <label class="campo"><span class="dato">Ripeti password</span><input type="password" bind:value={password2} required autocomplete="new-password" /></label>
        {:else if passo === 2}
          <label class="campo"><span class="dato">Nome istanza</span><input bind:value={istanza} maxlength="200" /></label>
          <label class="campo"><span class="dato">Indirizzo pubblico</span><input class="mono" type="url" bind:value={indirizzo} maxlength="200" placeholder="https://" />
            <span class="nota">L’indirizzo del tunnel Cloudflare, usato nei link di invito.</span></label>
          <label class="campo"><span class="dato">Anno scolastico</span><input class="mono" bind:value={annoScolastico} maxlength="200" /></label>
          <label class="campo"><span class="dato">Fuso orario</span><input class="mono" bind:value={fuso} maxlength="200" /></label>
        {:else if passo === 3}
          <div>
            <ul class="elenco">
              {#each app.meta?.groups ?? [] as g (g.id)}
                <li><span class="serif grande">{g.name}</span><span class="dato tenue">{g.kind}</span></li>
              {/each}
            </ul>
            <button type="button" class="pulsante tratteggiato aggiungi" onclick={() => (nuovoGruppo = true)}>+ Aggiungi classe o gruppo</button>
            {#each inviti as i (i.id)}
              <div class="riquadro">
                <span class="dato">Codice invito {app.meta?.groups.find((g) => g.id === i.group_id)?.name}</span>
                <span class="codice"><span>{codiceLeggibile(i.code)}</span><button type="button" class="collegamento" onclick={() => copia(i.code)}>Copia</button></span>
                <span class="nota">Scade tra 7 giorni. Chi entra con il codice è Alunno e lo confermi tu; i ruoli li cambi dopo.</span>
              </div>
            {/each}
          </div>
        {:else if passo === 4}
          <div class="blocco">
            <span class="dato">Materie · tag colorati</span>
            <div class="scelte">
              {#each app.meta?.tags ?? [] as m (m.id)}<span class="scelta"><i class="quadratino grande" style:background={m.color}></i>{m.name}</span>{/each}
              <button type="button" class="scelta nuova" onclick={() => (nuovo = 'materia')}>+ Materia</button>
            </div>
          </div>
          <div class="blocco">
            <span class="dato">Etichette di tipo</span>
            <div class="scelte">
              {#each app.meta?.labels ?? [] as t (t.id)}<span class="scelta tipo">{t.name}</span>{/each}
              <button type="button" class="scelta nuova" onclick={() => (nuovo = 'tipo')}>+ Tipo</button>
            </div>
            <span class="nota">Puoi aggiungerne altre in qualsiasi momento.</span>
          </div>
        {:else if stato}
          <div>
            <Interruttore nome="Dati cifrati sul disco" descrizione={stato.encrypted_at_rest ? 'Il database è cifrato con SQLCipher' : 'Questa build non cifra il database; gli eventi privati restano cifrati'} attivo={stato.encrypted_at_rest} disabilitato />
            <Interruttore nome="Protezione da abusi" descrizione="Al massimo {stato.login_attempts_per_minute} tentativi di accesso al minuto per utente" attivo disabilitato />
            <Interruttore nome="Origine bloccata sul tunnel" descrizione={stato.origin_pinned ? 'Le modifiche arrivano solo dall’indirizzo pubblico' : 'Imposta TASKHIT_PUBLIC_URL per attivarla'} attivo={stato.origin_pinned} disabilitato />
            <div class="backup"><span class="voce"><span>Backup automatici</span><span class="nota">In arrivo. Per ora salva la cartella dei dati.</span></span></div>
          </div>
        {/if}
        {#if errore}<p class="errore" role="alert">{errore}</p>{/if}
      </form>
    </div>

    <footer>
      <div>
        <!-- Dopo la creazione dell'admin non si torna al primo passo. -->
        <button type="button" class="indietro" disabled={passo <= 2 || occupato} onclick={() => { errore = ''; passo--; }}>Indietro</button>
        <button form="passo" class="pulsante grande {passo === 5 ? 'accento' : 'pieno'}" disabled={occupato} aria-busy={occupato}>
          {#if occupato}{passo === 1 ? 'Derivo le chiavi…' : 'Attendi…'}
          {:else if passo === 5}Apri TaskHit
          {:else}Continua<Icona nome="avanti" size={18} spessore={1.6} />{/if}
        </button>
      </div>
    </footer>
  </div>

  {#if nuovoGruppo}<NuovoGruppo onclose={() => (nuovoGruppo = false)} onfatto={caricaInviti} />{/if}
  {#if nuovo}
    <Foglio titolo={nuovo === 'materia' ? 'Nuova materia' : 'Nuovo tipo'} onclose={() => (nuovo = '')}>
      <form class="breve" onsubmit={aggiungi}>
        <label class="campo"><span class="dato">Nome</span><input bind:value={nomeNuovo} required maxlength="40" /></label>
        {#if nuovo === 'materia'}<label class="colore"><span class="dato">Colore</span><input type="color" bind:value={colore} /></label>{/if}
        <button class="pulsante pieno grande">Aggiungi</button>
      </form>
    </Foglio>
  {/if}
{/if}

<style>
  .pagina { min-height: 100dvh; display: flex; flex-direction: column; }
  .setup { flex: 1; max-width: 480px; padding-bottom: 28px; }
  .testa { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; }
  .testa .marchio { font-size: 26px; }
  .testa .dato { letter-spacing: .1em; }
  .avanzamento { display: flex; flex-direction: column; gap: 8px; }
  .avanzamento div { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 4px; }
  .avanzamento i { height: 3px; background: var(--filetto); }
  .avanzamento i.fatto { background: var(--inchiostro); }
  header { display: flex; flex-direction: column; gap: 8px; }
  h1 { font-size: 44px; line-height: .98; text-wrap: balance; }
  form { display: flex; flex-direction: column; gap: 16px; }
  .campo .nota { font-size: 12px; padding-top: 4px; }
  .elenco > li { min-height: 54px; }
  .grande.serif { font-size: 26px; }
  .aggiungi { width: 100%; margin-top: 12px; min-height: 44px; }
  .riquadro { margin-top: 18px; padding: 12px 14px; }
  .codice { display: flex; align-items: center; justify-content: space-between; gap: 8px; font-family: var(--font-dati); font-size: 22px; letter-spacing: .12em; }
  .codice .collegamento { font-family: var(--font-testo); letter-spacing: 0; }
  .blocco { display: flex; flex-direction: column; gap: 10px; padding-bottom: 4px; }
  span.scelta { min-height: 34px; cursor: default; }
  span.scelta.tipo { min-height: 30px; padding: 0 10px; }
  .backup { min-height: 56px; display: flex; align-items: center; border-bottom: 1px solid var(--filetto); }
  footer { position: sticky; bottom: 0; background: var(--sfondo); border-top: 1px solid var(--filetto); }
  footer div { max-width: 480px; margin: 0 auto; display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 14px 24px calc(18px + var(--basso)); }
  footer .pulsante { flex: 1; max-width: 220px; }
  .indietro { min-height: 52px; padding: 0 6px; font-size: 15px; }
  .breve { gap: 18px; }
  .colore { display: flex; align-items: center; justify-content: space-between; }
  .colore input { width: 64px; height: 44px; padding: 2px; border: 1px solid var(--filetto); background: var(--superficie); }
</style>
