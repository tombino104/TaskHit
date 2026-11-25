// Schermo largo (barra laterale) o stretto: stessa soglia usata nel CSS.
const query = matchMedia('(min-width: 900px)');
export const largo = $state({ attivo: query.matches });
query.addEventListener('change', () => { largo.attivo = query.matches; });
