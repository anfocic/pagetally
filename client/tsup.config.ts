import { defineConfig } from 'tsup'

export default defineConfig([
  // npm package: ESM + CJS + types, for bundled apps that `import { Analytics }`.
  {
    entry: ['src/index.ts'],
    format: ['esm', 'cjs'],
    dts: true,
    clean: true,
    sourcemap: true,
    minify: true,
    treeshake: true,
  },
  // Self-hosted <script> tag: a single self-initializing IIFE (dist/pt.js) that
  // the Rust server embeds and serves at /pt.js. es2019 keeps old browsers happy.
  {
    entry: { pt: 'src/auto.ts' },
    format: ['iife'],
    dts: false,
    clean: false,
    sourcemap: false,
    minify: true,
    treeshake: true,
    target: 'es2019',
    outExtension: () => ({ js: '.js' }),
  },
])
