// @ts-check
import { readFileSync } from 'node:fs';
import casoonPages from '@casoon/pages-theme';
import { defineConfig } from 'astro/config';

// The workspace manifest carries no version; the app crate does.
const version = readFileSync(new URL('../apps/desktop/Cargo.toml', import.meta.url), 'utf8').match(
  /^version = "(.+)"/m
)?.[1];

// Project page: https://casoon.github.io/devmark/ — `base` is the GitHub Pages path.
export default defineConfig({
  site: 'https://casoon.github.io/devmark',
  base: '/devmark/',
  integrations: [
    casoonPages({
      name: 'DevMark',
      description:
        'Desktop-Editor für Code, Notizen und längere Texte – Markdown und Typst mit Live-Druckvorschau und PDF-Export, Notizen als Markdown-Dateien. Rust, Local First.',
      tagline: 'Editor für Code, Notizen und Texte',
      repo: 'casoon/devmark',
      branch: 'master',
      version,
      license: 'Apache-2.0 OR MIT',
      lang: 'de',
      catalog: false,
      showcase: false,
      docsGroups: {
        einstieg: 'Einstieg',
        anleitungen: 'Anleitungen',
        referenz: 'Referenz',
      },
    }),
  ],
});
