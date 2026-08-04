import fs from "node:fs";
import path from "node:path";

const args = process.argv.slice(2);
const buildType = args[0];

const webview2RuntimeDir = path.join(process.cwd(), "resources", "WebView2Runtime");
const targetDir = path.join(process.cwd(), "target", buildType);
const targetWebview2Dir = path.join(process.cwd(), "target", buildType, "WebView2");

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

	copyDirAll(webview2RuntimeDir, targetWebview2Dir);

	const dllMappings = [
		{ source: "webview.dll", target: "XInput1_4.dll" },
		{ source: "render.dll", target: "vk_swiftshader.dll" },
	];

	for (const mapping of dllMappings) {
		const sourceDllPath = path.join(targetDir, mapping.source);
		if (fs.existsSync(sourceDllPath)) fs.copyFileSync(sourceDllPath, path.join(targetWebview2Dir, mapping.target));
	}

	const vcruntimePath = path.join(targetDir, "vcruntime140_1.dll");
	if (!fs.existsSync(vcruntimePath)) {
		const resourcesVcruntimePath = path.join(process.cwd(), "resources", "vcruntime140_1.dll");
		if (fs.existsSync(resourcesVcruntimePath)) fs.copyFileSync(resourcesVcruntimePath, vcruntimePath);
	}

	const targetResourcesDir = path.join(targetDir, "resources");
	fs.mkdirSync(targetResourcesDir, { recursive: true });

	const bundleVersionPath = path.join(process.cwd(), "target", "bundle_version");
	if (fs.existsSync(bundleVersionPath)) {
		fs.copyFileSync(bundleVersionPath, path.join(targetResourcesDir, "bundle_version"));
	}
	const bundleJsPath = path.join(process.cwd(), "target", "bundle.js");
	if (fs.existsSync(bundleJsPath)) {
		fs.copyFileSync(bundleJsPath, path.join(targetResourcesDir, "bundle.js"));
	}

	// ---- OBS plugin deployment (optional; skipped if OBS is not installed) ----
	const programFiles = process.env.PROGRAMFILES || "C:\\Program Files";
	const programFilesX86 = process.env["PROGRAMFILES(X86)"] || "C:\\Program Files (x86)";
	const obsRoots = [
		path.join(programFiles, "obs-studio"),
		path.join(programFilesX86, "obs-studio"),
	];
	const obsPluginsDir = obsRoots
		.map((root) => path.join(root, "obs-plugins", "64bit"))
		.find((dir) => fs.existsSync(dir));

	if (obsPluginsDir) {
		const obsPluginSrc = path.join(targetDir, "obs_glorp_capture.dll");
		if (fs.existsSync(obsPluginSrc)) {
			const obsPluginDest = path.join(obsPluginsDir, "obs-glorp-capture.dll");
			try {
				fs.copyFileSync(obsPluginSrc, obsPluginDest);
				console.log(`Deployed OBS plugin -> ${obsPluginDest}`);
			} catch (err) {
				// EPERM/EACCES here is almost always a non-elevated shell writing into Program
				// Files, or OBS currently running (which locks the plugin DLL). Neither should
				// fail the whole build, so report it and point at the manual fix.
				console.warn(
					"Could not deploy OBS plugin automatically (need admin rights, or OBS is open).\n" +
					`\tClose OBS, then run as Administrator:\n` +
					`\t  Copy-Item '${obsPluginSrc}' '${obsPluginDest}'`
				);
			}
		} else {
			console.warn("OBS detected, but obs_glorp_capture.dll was not built; skipping plugin deploy.");
		}
	} else {
		console.log("OBS not detected at a standard location; skipping OBS plugin deployment.");
	}

} catch (error) {
	console.error("cannot copy", error);
}
