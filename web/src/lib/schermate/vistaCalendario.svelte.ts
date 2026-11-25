// Giorno e vista scelti restano gli stessi quando si apre un evento e si torna indietro.
export const stato = $state({
  giorno: null as string | null,
  /** null = settimana sugli schermi larghi, mese sul telefono. */
  vista: null as 'giorno' | 'settimana' | 'mese' | null,
  filtriAperti: false,
});
