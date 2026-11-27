// La foto profilo si prepara sul dispositivo: ritaglio quadrato, 320 px, JPEG. Ridisegnarla
// elimina anche i dati nascosti nel file originale (posizione, fotocamera).
import { b64 } from './crypto';

const LATO = 320;
const MASSIMO = 150_000;

export const urlFoto = (id: string, versione: number): string => `/api/users/${encodeURIComponent(id)}/photo?v=${versione}`;

export async function riduciFoto(file: Blob): Promise<string> {
  let immagine: ImageBitmap;
  try { immagine = await createImageBitmap(file); } catch { throw new Error('Questo file non è un’immagine leggibile.'); }
  const taglio = Math.min(immagine.width, immagine.height);
  const tela = document.createElement('canvas');
  tela.width = tela.height = Math.min(LATO, taglio);
  const c = tela.getContext('2d')!;
  c.fillStyle = '#fff';
  c.fillRect(0, 0, tela.width, tela.height);
  c.drawImage(immagine, (immagine.width - taglio) / 2, (immagine.height - taglio) / 2, taglio, taglio, 0, 0, tela.width, tela.height);
  immagine.close();
  for (const qualita of [0.86, 0.75, 0.6, 0.45]) {
    const esito = await new Promise<Blob | null>((fatto) => tela.toBlob(fatto, 'image/jpeg', qualita));
    if (esito && esito.size <= MASSIMO) return b64.enc(new Uint8Array(await esito.arrayBuffer()));
  }
  throw new Error('Non riesco a ridurre abbastanza questa foto.');
}
