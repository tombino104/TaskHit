<script lang="ts">
  // In HTTP (fuori da localhost) il browser non offre WebCrypto: niente account, accesso o eventi privati.
  const indirizzo = location.origin;
</script>

<div class="schermata">
  <div class="testa">
    <span class="marchio">TaskHit</span>
    <span class="dato">Connessione non sicura</span>
  </div>
  <header>
    <h1>Serve HTTPS</h1>
    <p class="lede">Senza HTTPS il browser disattiva la crittografia che protegge password ed eventi privati. Da questo indirizzo non puoi creare un account né accedere.</p>
  </header>

  <dl class="scheda-dati">
    <dt>Sei su</dt>
    <dd class="mono">{indirizzo}</dd>
  </dl>

  <div class="riquadro">
    <span class="dato">Cosa fare</span>
    <p>Apri TaskHit dal suo indirizzo che inizia con <span class="mono">https://</span>, per esempio quello del tunnel Cloudflare.</p>
    <p class="nota">Se non ne hai ancora uno, chi gestisce il server deve configurarlo.</p>
  </div>
</div>

<style>
  .schermata { max-width: 480px; }
  .testa { display: flex; align-items: baseline; justify-content: space-between; border-bottom: 1px solid var(--inchiostro); padding-bottom: 12px; }
  header { display: flex; flex-direction: column; gap: 8px; padding-top: 12px; }
  h1 { font-size: 44px; line-height: .98; text-wrap: balance; }
</style>
