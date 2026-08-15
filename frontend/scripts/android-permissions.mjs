// 给生成的安卓工程补上摄像头权限。
//
// 扫码用的是 WebView 里的 getUserMedia。wry 的 WebChromeClient 收到
// VIDEO_CAPTURE 请求时会去申请 CAMERA 运行时权限,但**清单里没声明的权限,
// 安卓一律直接判拒**,不会弹窗 —— 网页那边只看到一个 NotAllowedError,很容易
// 被当成"页面不是 HTTPS"。应用页面来源是 http://tauri.localhost,`.localhost`
// 在 Chromium 里算可信来源,本来就是 secure context,与 HTTPS 无关。
//
// `tauri android init` 会重新生成 AndroidManifest.xml,手改留不住,所以这个
// 脚本挂在 init 之后、build 之前跑(见 package.json 和 release.yml)。构建
// 本身不会重写这个文件 —— 会被重写的那几个都列在 gen/android/app/.gitignore 里。

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const MANIFEST = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "src-tauri/gen/android/app/src/main/AndroidManifest.xml",
);

// required="false" 是有意的:没有摄像头的设备(平板、模拟器)照样能装,只是扫不了码。
const ADDITIONS = [
  '    <uses-permission android:name="android.permission.CAMERA" />',
  '    <uses-feature android:name="android.hardware.camera" android:required="false" />',
];

let manifest;
try {
  manifest = readFileSync(MANIFEST, "utf8");
} catch (error) {
  if (error.code === "ENOENT") {
    console.error(`找不到 ${MANIFEST}。先跑 \`npm run tauri:android:init\`。`);
    process.exit(1);
  }
  throw error;
}

const missing = ADDITIONS.filter((line) => !manifest.includes(line.trim()));
if (missing.length === 0) {
  console.log("安卓清单已包含摄像头权限,跳过。");
  process.exit(0);
}

const anchor = '<uses-permission android:name="android.permission.INTERNET" />';
if (!manifest.includes(anchor)) {
  console.error("安卓清单里没有预期的 INTERNET 权限声明,模板可能变了,请人工检查。");
  process.exit(1);
}

writeFileSync(
  MANIFEST,
  manifest.replace(anchor, [anchor, ...missing].join("\n")),
  "utf8",
);
console.log(`已写入摄像头权限(${missing.length} 行)。`);
