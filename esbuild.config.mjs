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

			// replace script tags with cause they get owned by the "minification"
			const preserved = [];
			let index = 0;

			contents = contents.replace(/<script\b[^>]*>([\s\S]*?)<\/script>/gi, (match) => {
				const placeholder = `___SCRIPT_${index}___`;
				preserved[index] = match;
				index++;
				return placeholder;
			});

			contents = contents.replace(/\s+/g, " ").trim();

			for (let i = 0; i < preserved.length; i++) {
				contents = contents.replace(`___SCRIPT_${i}___`, preserved[i]);
				contents = contents.replace(`___STYLE_${i}___`, preserved[i]);
			}

			return { loader: "text", contents };
		});
	},
};

export const minifyInlinePlugin = {
	name: "minifyInlinePlugin",
	setup(build) {
		build.onLoad({ filter: /\.js$/ }, async (args) => {
			let contents = await readFile(args.path, "utf8");

			// find /* css */ or /* html */ comments in js files and parse them
			const cssRegex = /\/\*\s*css\s*\*\/\s*`([\s\S]*?)`/g;
			const htmlRegex = /\/\*\s*html\s*\*\/\s*`([\s\S]*?)`/g;
			const cssMatches = [...contents.matchAll(cssRegex)];
			if (cssMatches.length > 0) {
				const transformPromises = cssMatches.map((match) => transform(match[1], { loader: "css", minify: true }));
				const results = await Promise.all(transformPromises);
				let index = 0;
				contents = contents.replace(cssRegex, () => `\`${results[index++].code}\``);
			}

			contents = contents.replace(htmlRegex, (html) => {
				const minified = html.replace(/\s+/g, " ").trim();
				return `${minified}`;
			});

			return { contents, loader: "js" };
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
	plugins: [textMinifyPlugin, minifyCSS, minifyInlinePlugin],
})
	.then(() => {
		console.log("Build completed successfully!");
	})
	.catch((error) => {
		console.error("Build failed:", error);
		process.exit(1);
	});
