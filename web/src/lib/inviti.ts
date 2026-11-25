// Codici invito: "K7Q4BX29M" si mostra come "K7Q-4BX-29M".
export const codiceLeggibile = (code: string): string => code.match(/.{1,3}/g)?.join('-') ?? code;

/** Il link usa l'indirizzo pubblico impostato dall'admin; se manca, quello da cui si sta navigando. */
export const linkInvito = (pubblico: string | undefined, code: string, origine = location.origin): string =>
  `${(pubblico || origine).replace(/\/+$/, '')}/#/entra/${codiceLeggibile(code)}`;
