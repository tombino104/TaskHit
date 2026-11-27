<script lang="ts">
  // Le azioni di moderazione su un account: usata dall'admin (Persone) e dai rappresentanti (La mia classe).
  import Foglio from './Foglio.svelte';
  import { api, type Azione, type Moderato } from '../api';
  import { app, avvisa, messaggio, sincronizza } from '../store.svelte';
  import { commuta } from '../events';
  import { DURATE, MOTIVI, nomiMotivi, statoBlocco } from '../moderazione';

  let { persona, onclose, onfatto }: { persona: Moderato; onclose: () => void; onfatto?: () => unknown } = $props();

  let azione = $state<Azione | ''>('');
  let motivi = $state<string[]>([]);
  let testo = $state('');
  let giorni = $state<number | null>(7);
  let errore = $state('');
  let occupato = $state(false);

  const AZIONI = $derived<[Azione, string][]>([
    ...(persona.block ? [['riattiva', 'Riattiva l’account'] as [Azione, string]] : []),
    ...(persona.block?.kind === 'espulso' ? [] : [
      ['sospendi', 'Sospendi'] as [Azione, string],
      ['nome', 'Chiedi di cambiare nome'] as [Azione, string],
      ...(persona.photo ? [['foto', 'Togli la foto'] as [Azione, string]] : []),
      ['espelli', 'Espelli'] as [Azione, string],
    ]),
  ]);
  const conMotivo = $derived(azione === 'sospendi' || azione === 'espelli' || azione === 'nome');
  const SPIEGA: Record<Azione, string> = {
    sospendi: 'Esce subito e non può entrare. Al prossimo accesso legge il motivo.',
    espelli: 'Esce dalla classe e l’account resta bloccato, con il motivo.',
    nome: 'Potrà solo leggere finché non sceglie un nome diverso.',
    foto: 'La foto viene cancellata. Potrà caricarne un’altra.',
    riattiva: 'Potrà di nuovo entrare. Se era espulso, l’admin deve rimetterlo in classe.',
  };

  async function conferma() {
    if (!azione) return;
    if (conMotivo && !motivi.length && !testo.trim()) { errore = 'Scrivi il motivo oppure scegline uno.'; return; }
    errore = ''; occupato = true;
    try {
      await api.sanziona(persona.id, {
        action: azione,
        ...(conMotivo ? { tags: $state.snapshot(motivi), reason: testo.trim() || undefined } : {}),
        ...(azione === 'sospendi' && giorni ? { days: giorni } : {}),
      });
      await sincronizza();
      await onfatto?.();
      avvisa('Fatto.');
      onclose();
    } catch (e) { errore = messaggio(e); } finally { occupato = false; }
  }
</script>

<Foglio titolo={persona.name} {onclose}>
  {#if persona.block}
    <p class="errore">{statoBlocco(persona.block)}{persona.block.tags.length ? ` · ${nomiMotivi(persona.block.tags)}` : ''}{persona.block.reason ? ` · «${persona.block.reason}»` : ''}</p>
  {/if}
  <fieldset>
    <legend class="dato">Cosa vuoi fare</legend>
    <div class="scelte" role="radiogroup" aria-label="Azione">
      {#each AZIONI as [id, nome] (id)}
        <button type="button" role="radio" class="scelta" aria-checked={azione === id} onclick={() => { azione = id; errore = ''; }}>{nome}</button>
      {/each}
    </div>
    {#if azione}<p class="nota spiega">{SPIEGA[azione]}</p>{/if}
  </fieldset>

  {#if conMotivo}
    <fieldset>
      <legend class="dato">Motivo · scegline uno o scrivilo</legend>
      {#each MOTIVI as m (m.id)}
        <label class="spunta"><input type="checkbox" checked={motivi.includes(m.id)} onchange={() => (motivi = commuta(motivi, m.id))} /><span>{m.nome}</span></label>
      {/each}
    </fieldset>
    <label><span class="dato">Spiegazione{motivi.length ? ' · facoltativa' : ' · obbligatoria'}</span>
      <textarea rows="3" bind:value={testo} maxlength="500" placeholder="La leggerà la persona interessata"></textarea></label>
  {/if}

  {#if azione === 'sospendi'}
    <fieldset>
      <legend class="dato">Durata</legend>
      <div class="scelte" role="radiogroup" aria-label="Durata">
        {#each DURATE as [n, nome] (nome)}<button type="button" role="radio" class="scelta" aria-checked={giorni === n} onclick={() => (giorni = n)}>{nome}</button>{/each}
      </div>
    </fieldset>
  {/if}

  {#if errore}<p class="errore" role="alert">{errore}</p>{/if}
  <button type="button" class="pulsante accento grande" disabled={!azione || occupato || !app.online} aria-busy={occupato} onclick={conferma}>
    {azione ? AZIONI.find(([id]) => id === azione)?.[1] : 'Scegli un’azione'}
  </button>
</Foglio>

<style>
  legend { padding: 0 0 8px; }
  .spiega { padding-top: 8px; }
  .spunta { display: flex; align-items: center; gap: 10px; min-height: 44px; border-bottom: 1px solid var(--filetto); }
  label textarea { margin-top: 6px; }
</style>
