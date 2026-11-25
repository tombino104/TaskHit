// Navigazione con l'hash (#/evento/…): funziona anche offline e non richiede librerie.
function leggi() {
  const [nome = '', arg = ''] = location.hash.replace(/^#\/?/, '').split('/');
  return { nome: nome || 'oggi', arg: decodeURIComponent(arg) };
}

export const rotta = $state(leggi());
let passi = 0;

addEventListener('hashchange', () => {
  Object.assign(rotta, leggi());
  passi++;
  scrollTo(0, 0);
});

export const vai = (percorso: string): void => { location.hash = `/${percorso}`; };
export const sostituisci = (percorso: string): void => { location.replace(`#/${percorso}`); };
/** Torna alla pagina precedente dell'app; se si è entrati da un link diretto, va a `altrimenti`. */
export const indietro = (altrimenti = 'oggi'): void => { if (passi > 0) history.back(); else sostituisci(altrimenti); };
