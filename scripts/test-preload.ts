import { plugin } from "bun";

// Vite's ?raw imports are file contents. Bun otherwise supplies an asset path,
// leaving the real translation catalogs empty in tests that import the web UI.
plugin({
  name: "vite-raw-test-assets",
  setup(build) {
    build.onLoad({ filter: /\.yml\?raw$/ }, async ({ path }) => ({
      contents: `export default ${JSON.stringify(await Bun.file(path.slice(0, -"?raw".length)).text())};`,
      loader: "js",
    }));
  },
});
