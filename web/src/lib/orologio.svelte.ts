// L'ora corrente, aggiornata ogni minuto: dopo mezzanotte «Oggi» cambia da sola.
export const orologio = $state({ adesso: new Date() });
const aggiorna = () => { orologio.adesso = new Date(); };
setInterval(aggiorna, 60_000);
document.addEventListener('visibilitychange', aggiorna);
