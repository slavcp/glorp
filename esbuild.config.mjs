import * as esbuild from "esbuild";
import { transform } from "esbuild";
import { readFile } from "node:fs/promises";

export const minifyCSS = {
	name: "minifyCSS",
	setup(build) {
		build.onLoad({ filter: /\.css$/ }, async (args) => {
			const file = await readFile(args.path);
			const css = await transform(file, { loader: "css", minify: true });
			return { loader: "text", contents: css.code };
		});
	},
};

console.log("Starting esbuild process...");
await esbuild
	.build({
		entryPoints: ["./src/frontend/main.js"],
		bundle: true,
		minify: true,
		format: "esm",
		treeShaking: true,
		minifyWhitespace: true,
		minifySyntax: true,
		ignoreAnnotations: true,
		loader: {
			".css": "text",
			".html": "text",
		},
		outfile: "./target/bundle.js",
		plugins: [minifyCSS],
	})
	.then(() => {
		console.log("Build completed successfully!");
	})
	.catch((error) => {
		console.error("Build failed:", error);
		process.exit(1);
	});
