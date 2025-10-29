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

		if (entry.isDirectory()) {
			copyDirAll(sourcePath, destPath);
		} else {
			fs.copyFileSync(sourcePath, destPath);
		}
	}
}

try {
	fs.mkdirSync(targetWebview2Dir, { recursive: true });

	copyDirAll(webview2RuntimeDir, targetWebview2Dir);

	const webviewDllPath = path.join(targetDir, "webview.dll");
	if (fs.existsSync(webviewDllPath)) {
		fs.copyFileSync(webviewDllPath, path.join(targetWebview2Dir, "XInput1_4.dll"));
	}

	const renderDllPath = path.join(targetDir, "render.dll");
	if (fs.existsSync(renderDllPath)) {
		fs.copyFileSync(renderDllPath, path.join(targetWebview2Dir, "vk_swiftshader.dll"));
	}
} catch (error) {
	console.error("cannot copy", error);
}
