import { defineConfig } from 'vite';
import path from 'path';

export default defineConfig({
  build: {
    outDir: path.resolve(__dirname, 'dist'),
    emptyOutDir: true,
    lib: {
      entry: path.resolve(__dirname, 'src/index.ts'),
      name: 'OxidViewer',
      formats: ['es', 'umd'],
      fileName: (format) => `oxid-viewer.${format}.js`,
    },
    rollupOptions: {
      output: {
        exports: 'named',
      },
    },
  },
});
