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
        'Desktop editor for code, notes and long-form writing – Markdown and Typst with a live print preview and PDF export, notes as plain Markdown files. Rust, local-first.',
      tagline: 'Editor for code, notes and writing',
      repo: 'casoon/devmark',
      branch: 'master',
      version,
      license: 'Apache-2.0 OR MIT',
      lang: 'en',
      catalog: false,
      showcase: false,
      docsGroups: {
        'getting-started': 'Getting started',
        guides: 'Guides',
        reference: 'Reference',
      },
    }),
  ],
});
