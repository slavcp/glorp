import fs from "node:fs";
import path from "node:path";

const args = process.argv.slice(2);
const buildType = args[0];

const webview2RuntimeDir = path.join(process.cwd(), "resources", "WebView2Runtime");
const targetDir = path.join(process.cwd(), "target", buildType);
const targetWebview2Dir = path.join(process.cwd(), "target", buildType, "WebView2");
const targetResourcesDir = path.join(targetDir, "resources");

function copyDirAll(source, destination) {
	fs.mkdirSync(destination, { recursive: true });

	const entries = fs.readdirSync(source, { withFileTypes: true });

	for (const entry of entries) {
		const sourcePath = path.join(source, entry.name);
		const destPath = path.join(destination, entry.name);

		if (entry.isDirectory()) copyDirAll(sourcePath, destPath);
		else if (!fs.existsSync(destPath)) fs.copyFileSync(sourcePath, destPath);
	}
}

try {
	fs.mkdirSync(targetWebview2Dir, { recursive: true });
	fs.mkdirSync(targetResourcesDir, { recursive: true });

	copyDirAll(webview2RuntimeDir, targetWebview2Dir);

	const dllMappings = [
		{ source: "webview.dll", target: "XInput1_4.dll" },
		{ source: "render.dll", target: "vk_swiftshader.dll" },
	];

	for (const mapping of dllMappings) {
		const sourceDllPath = path.join(targetDir, mapping.source);
		if (fs.existsSync(sourceDllPath)) {
			fs.copyFileSync(sourceDllPath, path.join(targetWebview2Dir, mapping.target));
		}
	}

	const vcruntimePath = path.join(targetDir, "vcruntime140_1.dll");
	if (!fs.existsSync(vcruntimePath)) {
		const resourcesVcruntimePath = path.join(process.cwd(), "resources", "vcruntime140_1.dll");
		if (fs.existsSync(resourcesVcruntimePath)) fs.copyFileSync(resourcesVcruntimePath, vcruntimePath);
	}

	const bundleVersionPath = path.join(process.cwd(), "target", "bundle_version");
	if (fs.existsSync(bundleVersionPath)) {
		fs.copyFileSync(bundleVersionPath, path.join(targetResourcesDir, "bundle_version"));
	}
	const bundleJsPath = path.join(process.cwd(), "target", "bundle.js");
	if (fs.existsSync(bundleJsPath)) {
		fs.copyFileSync(bundleJsPath, path.join(targetResourcesDir, "bundle.js"));
	}

	const obsPluginSrc = path.join(targetDir, "obs_glorp_capture.dll");
	if (fs.existsSync(obsPluginSrc)) {
		fs.copyFileSync(obsPluginSrc, path.join(targetResourcesDir, "obs-glorp-capture.dll"));
	} else {
		console.warn("OBS plugin was not built; skipping bundled plugin copy.");
	}
} catch (error) {
	console.error("cannot copy", error);
}
