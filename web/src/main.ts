import '@fontsource/instrument-serif/400.css';
import '@fontsource/instrument-serif/400-italic.css';
import '@fontsource/geist/400.css';
import '@fontsource/geist/500.css';
import '@fontsource/geist/600.css';
import '@fontsource/geist-mono/400.css';
import '@fontsource/geist-mono/500.css';
import './app.css';
import { mount } from 'svelte';
import { preparaLingua } from './lib/i18n';

// L'interfaccia si carica dopo la lingua: così anche i testi preparati all'avvio sono tradotti.
preparaLingua()
  .then(() => import('./App.svelte'))
  .then(({ default: App }) => mount(App, { target: document.getElementById('app')! }));
