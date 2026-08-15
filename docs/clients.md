# 客户端

除了浏览器直接访问,还提供桌面端(Windows / macOS / Linux)和安卓端。它们跟网页版是**同一套前端代码**,只是外壳不同——功能没有差别。

## 什么时候需要客户端

浏览器已经够用的话就不需要装。客户端主要解决两件事:

- **扫码**。浏览器要拿摄像头权限,页面必须是 HTTPS 或 localhost。内网纯 HTTP 部署时网页版扫不了码,客户端可以
- **手机上更顺手**。安卓端是原生应用,不用每次开浏览器输地址

## 下载

[GitHub Releases](https://github.com/spinleft/lab-inventory/releases) 页面,按平台选:

| 平台 | 文件 |
| --- | --- |
| Windows | `lab-inventory-<版本>-windows-x86_64.msi` |
| macOS(Apple Silicon) | `lab-inventory-<版本>-macos-aarch64.dmg` |
| macOS(Intel) | `lab-inventory-<版本>-macos-x86_64.dmg` |
| Linux | `lab-inventory-<版本>-linux-x86_64.deb` / `.AppImage` |
| 安卓 | `lab-inventory-<版本>-android-universal.apk` |

### 关于签名

0.1 版本的安装包**没有代码签名**:

- **Windows** 会弹 SmartScreen 警告。点"更多信息"→"仍要运行"
- **macOS** 会说"无法验证开发者"。在"系统设置 → 隐私与安全性"里点"仍要打开",或者:
  ```bash
  xattr -dr com.apple.quarantine /Applications/Lab\ Inventory.app
  ```
- **安卓** APK 如果文件名里带 `-unsigned`(比如 `lab-inventory-0.1.0-android-universal-unsigned.apk`),表示构建时没有配置签名密钥,**这样的包装不上**。需要自己签名后再安装(见下)

签名证书是要花钱的,开源项目短期内不一定有。介意的话可以自己从源码构建。

## 手机上的界面

宽度小于 768px 时是另一套界面,不是把桌面布局压扁:

- 底部 tab 导航,按当前账号权限挑四项 + "更多";顶栏是标题、返回和实验室切换
- 列表按卡片排布,行操作收进卡片右上角的菜单
- 筛选收进底部 sheet,按钮上标着当前生效的条件数
- 对话框从底部升起,控件按触控尺寸放大,避开刘海屏和手势条

平板和桌面保持侧边栏与表格 —— 那个宽度下表格的信息密度才是优势。

## 首次配置

客户端启动后第一件事是告诉它后端在哪:

1. 打开应用,会看到"服务器设置"页面
2. 填后端地址,比如 `https://inventory.example.com`(会自动补上 `/api/v1`)
3. 保存,然后登录

地址存在本机,换服务器时在设置里改。

### 不需要配跨域

客户端的 API 请求不走 WebView 的 `fetch`,而是交给外壳的原生 HTTP 层(Tauri 的 http 插件)发出。它不受浏览器同源策略约束,**服务端不需要为客户端放行任何来源**。

真正的原因是 Cookie 而不是 CORS:客户端页面的来源是 `http://tauri.localhost`,相对任何后端地址都算跨站,WebView 不会把后端下发的 `SameSite=Lax` 会话 Cookie 带上——表现是登录本身成功,紧接着的请求 401,人被弹回登录页。原生 HTTP 层有自己的 Cookie 存储,不受这条规则影响。

`CORS_ALLOWED_ORIGINS` 只对**浏览器访问**有意义:前端和后端不同源时(比如前端在 `:5173`、后端在 `:8000`),要把前端地址加进去。

## 安卓 APK 自己签名

如果拿到的是 `-unsigned` 的包:

```bash
# 一次性:生成密钥库
keytool -genkey -v -keystore my-release.jks \
  -keyalg RSA -keysize 2048 -validity 10000 -alias lab-inventory

# 对齐并签名
zipalign -v -p 4 lab-inventory-0.1.0-android-universal-unsigned.apk aligned.apk
apksigner sign --ks my-release.jks --out lab-inventory-signed.apk aligned.apk
```

`zipalign` 和 `apksigner` 在 Android SDK 的 `build-tools` 目录里。

密钥库要保管好:安卓要求同一个应用的后续版本必须用同一个密钥签名,弄丢了就只能让用户卸载重装。

## 自己构建

### 桌面端

需要 Rust、Node.js 22+,以及 [Tauri 的系统依赖](https://tauri.app/start/prerequisites/)。

```bash
cd frontend
npm ci
npm run tauri:build
```

产物在 `frontend/src-tauri/target/release/bundle/` 下。

把后端地址烘进构建产物,省去用户手动配置:

```bash
VITE_DEFAULT_API_BASE_URL=https://inventory.example.com/api/v1 npm run tauri:build
```

单位内部分发时这样最省事——装上就能用。

### 安卓端

额外需要 JDK 17、Android SDK 和 NDK,并设好 `ANDROID_HOME` 和 `NDK_HOME`。

```bash
cd frontend
npm ci
npm run tauri:android:init      # 生成安卓工程,只需要第一次
npm run tauri:android:build
```

APK 在 `frontend/src-tauri/gen/android/app/build/outputs/apk/` 下。

`gen/` 目录是生成的,不在版本库里,可以随时删掉重新 init。

签名配置写在 `src-tauri/gen/android/keystore.properties`:

```properties
storeFile=release.jks
storePassword=<口令>
keyAlias=lab-inventory
password=<口令>
```

这个文件和 `.jks` 都**不要提交进 git**。

## 已知限制

- **离线不可用**。所有数据都是实时从服务器取的,断网就用不了
- **没有自动更新**。新版本要手动下载安装
- **iOS 没有发布**。代码里有 Tauri 的 iOS 支持,但没有构建和分发——上架 App Store 需要开发者账号
