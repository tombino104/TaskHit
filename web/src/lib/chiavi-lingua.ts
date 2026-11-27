// Serve solo ai test: trova nel codice tutti i testi da tradurre, così un file di lingua non resta indietro.

/** Legge una stringa tra apici a partire da `i`; restituisce il contenuto e dove finisce. */
function stringa(s: string, i: number): [string, number] {
  const apice = s[i]!;
  let out = '';
  for (i++; i < s.length && s[i] !== apice; i++) {
    if (s[i] === '\\') { const c = s[++i]!; out += c === 'n' ? '\n' : c; } else out += s[i];
  }
  return [out, i];
}

/** I testi nel primo argomento di ogni `t(...)`: anche entrambi i rami di una scelta `a ? 'x' : 'y'`. */
export function chiaviWeb(sorgente: string): string[] {
  const out: string[] = [];
  for (const m of sorgente.matchAll(/(?<![\w.$])t\(/g)) {
    let profondita = 0;
    for (let i = m.index + m[0].length; i < sorgente.length; i++) {
      const c = sorgente[i]!;
      if (c === "'" || c === '"') {
        const [testo, fine] = stringa(sorgente, i);
        const prima = sorgente.slice(Math.max(0, i - 5), i).trimEnd();
        const dopo = sorgente.slice(fine + 1, fine + 6).trimStart();
        // Un testo confrontato (`x === 'sett'`) non è da tradurre.
        if (profondita === 0 && !prima.endsWith('==') && !/^[!=]=/.test(dopo)) out.push(testo);
        i = fine;
      } else if ('([{'.includes(c)) profondita++;
      else if (')]}'.includes(c)) { if (profondita-- === 0) break; }
      else if (c === ',' && profondita === 0) break;
    }
  }
  return out;
}

/** I messaggi d'errore che il server manda all'interfaccia. */
export function chiaviServer(sorgente: string): string[] {
  const codice = sorgente.split('#[cfg(test)]')[0]!;
  const schemi = [
    /(?:BadRequest|Forbidden|Conflict)\(\s*"((?:[^"\\]|\\.)*)"/g,
    /(?:required_text|optional_text)\([^;"]*"((?:[^"\\]|\\.)*)"\s*\)/g,
    /const [A-Z_]+: (?:&str|ApiError) =\s*(?:ApiError::[A-Za-z]+\()?"((?:[^"\\]|\\.)*)"/g,
    /ApiError::(?:Unauthorized|NotFound|TooManyRequests|Unavailable) =>[^"]*?"((?:[^"\\]|\\.)*)"\)/g,
    /"error": "((?:[^"\\]|\\.)*)"/g,
    /INTERNAL_SERVER_ERROR, "((?:[^"\\]|\\.)*)"/g,
  ];
  return schemi.flatMap((re) => [...codice.matchAll(re)].map((m) => m[1]!.replace(/\\(.)/g, '$1')))
    .filter((m) => !/^[a-z]\./.test(m)); // elenchi di colonne SQL
}
