import { readFile } from "node:fs/promises";
import { build, transform } from "esbuild";

export const minifyCSS = {
	name: "minifyCSS",
	setup(build) {
		build.onLoad({ filter: /\.css$/ }, async (args) => {
			const file = await readFile(args.path, "utf8");
			const css = await transform(file, { loader: "css", minify: true });
			return { loader: "text", contents: css.code };
		});
	},
};

export const textMinifyPlugin = {
	name: "textMinifyPlugin",
	setup(build) {
		build.onLoad({ filter: /\.html$/ }, async (args) => {
			let contents = await readFile(args.path, "utf8");

			const scripts = [];
			let index = 0;

			// replace all script tags with placeholders cause they get owned by my whitespace remover 3000
			contents = contents.replace(/<script[^>]*>([\s\S]*?)<\/script>/gi, (_, scriptContent) => {
				scripts.push(scriptContent);
				return `___SCRIPT_${index++}___`;
			});

			for (let i = 0; i < scripts.length; i++) {
				const transformed = await transform(scripts[i], { loader: "js", minify: true });
				scripts[i] = transformed.code;
			}

			// holy minifier
			contents = contents.replace(/\s+/g, " ").trim();

			// reinstert script tags
			for (let i = 0; i < scripts.length; i++) {
				const minifiedCode = scripts[i];
				contents = contents.replace(`___SCRIPT_${i}___`, `<script>${minifiedCode}</script>`);
			}

			return { loader: "text", contents };
		});
	},
};

console.log("Starting esbuild process...");
await build({
	entryPoints: ["./src/frontend/main.js"],
	bundle: true,
	minify: true,
	format: "esm",
	treeShaking: true,
	minifyWhitespace: true,
	minifySyntax: true,
	ignoreAnnotations: true,
	loader: {
		".html": "text",
	},
	outfile: "./target/bundle.js",
	plugins: [textMinifyPlugin, minifyCSS],
})
	.then(() => {
		console.log("Build completed successfully!");
	})
	.catch((error) => {
		console.error("Build failed:", error);
		process.exit(1);
	});
