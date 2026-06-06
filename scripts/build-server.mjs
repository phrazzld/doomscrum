import { build } from 'esbuild';

const shared = {
  bundle: true,
  platform: 'node',
  format: 'esm',
  target: 'node22',
  sourcemap: true,
  packages: 'external',
  logLevel: 'info'
};

await Promise.all([
  build({
    ...shared,
    entryPoints: ['src/server/start.ts'],
    outfile: 'build-server/start.js'
  }),
  build({
    ...shared,
    entryPoints: ['src/cli/generate.ts'],
    outfile: 'build-server/generate.js'
  }),
  build({
    ...shared,
    entryPoints: ['src/cli/report.ts'],
    outfile: 'build-server/report.js'
  })
]);
