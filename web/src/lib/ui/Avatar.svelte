<script lang="ts">
  import { iniziali } from '../events';
  let { nome, size = 36, tono = 'tenue' }: { nome: string; size?: number; tono?: 'tenue' | 'pieno' | 'neutro' } = $props();
</script>

<span class={tono} style:width="{size}px" style:height="{size}px" style:font-size="{Math.max(12, size * 0.36)}px" aria-hidden="true">{iniziali(nome)}</span>

<style>
  span { flex: none; border-radius: 50%; display: inline-flex; align-items: center; justify-content: center; font-weight: 600; line-height: 1; }
  .tenue { background: var(--accento-tenue); color: var(--accento); }
  .pieno { background: var(--inchiostro); color: var(--sfondo); }
  .neutro { background: var(--superficie); color: var(--inchiostro); border: 1px solid var(--filetto); }
</style>
