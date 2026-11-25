<script lang="ts" module>
  // Icone a linea, disegnate nel codice: nessuna dipendenza e nessuna richiesta esterna.
  const ICONE = {
    cerca: '<circle cx="11" cy="11" r="6"/><path d="M20 20l-4.5-4.5"/>',
    sinistra: '<path d="M15 5l-7 7 7 7"/>',
    destra: '<path d="M9 5l7 7-7 7"/>',
    giu: '<path d="M6 9l6 6 6-6"/>',
    avanti: '<path d="M5 12h14M13 6l6 6-6 6"/>',
    piu: '<path d="M12 5v14M5 12h14"/>',
    meno: '<path d="M5 12h14"/>',
    chiudi: '<path d="M6 6l12 12M18 6L6 18"/>',
    spunta: '<path d="M5 12.5l4.5 4.5L19 7"/>',
    altro: '<g fill="currentColor" stroke="none"><circle cx="6" cy="12" r="1.4"/><circle cx="12" cy="12" r="1.4"/><circle cx="18" cy="12" r="1.4"/></g>',
    globo: '<circle cx="12" cy="12" r="8"/><path d="M4 12h16M12 4c2.5 2.5 2.5 13.5 0 16M12 4c-2.5 2.5-2.5 13.5 0 16"/>',
    lucchetto: '<rect x="5" y="11" width="14" height="9" rx="1"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/>',
    oggi: '<rect x="4" y="4" width="16" height="16" rx="1"/><path d="M8 9h8M8 13h8M8 17h5"/>',
    calendario: '<rect x="3.5" y="5" width="17" height="15" rx="1"/><path d="M3.5 10h17M8 3v4M16 3v4"/>',
    inbox: '<path d="M4 13l2.5-8h11L20 13v6H4z"/><path d="M4 13h5l1 2h4l1-2h5"/>',
    profilo: '<circle cx="12" cy="8.5" r="3.5"/><path d="M5 20c1.2-3.5 4-5 7-5s5.8 1.5 7 5"/>',
    filtro: '<path d="M4 6h16M7 12h10M10 18h4"/>',
    matita: '<path d="M4 20h4L19 9l-4-4L4 16z"/>',
    campana: '<path d="M6 16v-5a6 6 0 0 1 12 0v5l1.5 2h-15z"/><path d="M10 20.5a2 2 0 0 0 4 0"/>',
    cestino: '<path d="M5 7h14M10 7V4h4v3M7 7l1 13h8l1-13"/>',
    info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/>',
    camera: '<path d="M4 8h3l2-3h6l2 3h3v11H4z"/><circle cx="12" cy="13" r="3.5"/>',
    carica: '<path d="M12 16V4M7 9l5-5 5 5M5 20h14"/>',
    aggiorna: '<path d="M20 12a8 8 0 1 1-2.3-5.6M20 4v4h-4"/>',
  };
  export type NomeIcona = keyof typeof ICONE;
</script>

<script lang="ts">
  let { nome, size = 22, spessore = 1.5 }: { nome: NomeIcona; size?: number; spessore?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width={spessore} stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <!-- Il contenuto è una costante del modulo, mai dati dell'utente. -->
  {@html ICONE[nome]}
</svg>
