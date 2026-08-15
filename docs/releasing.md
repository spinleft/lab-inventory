# 发版

给维护者看的:怎么发一个新版本。

## 版本号

遵循 [语义化版本](https://semver.org/lang/zh-CN/)。0.x 阶段次版本号可以带不兼容变更,主版本号保持 0。

版本号出现在四个地方,**必须一致**,否则发布流程的第一步就会失败:

| 文件                                   | 字段          |
| -------------------------------------- | ------------- |
| `backend/Cargo.toml`                 | `version`   |
| `frontend/package.json`              | `version`   |
| `frontend/src-tauri/tauri.conf.json` | `version`   |
| git tag                                | `v<版本号>` |

## 流程

### 1. 确认 main 是绿的

[Actions 页面](https://github.com/spinleft/lab-inventory/actions)上 CI 全过。

### 2. 改版本号

```bash
VERSION=0.2.0

sed -i "0,/^version = .*/s//version = \"$VERSION\"/" backend/Cargo.toml
cd frontend && npm version "$VERSION" --no-git-tag-version && cd ..
sed -i "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" frontend/src-tauri/tauri.conf.json

# Cargo.lock 里的版本号也要跟着更新
cd backend && cargo check --quiet && cd ..
```

### 3. 写 CHANGELOG

在 `CHANGELOG.md` 顶上加一节。标题格式必须是 `## [<版本号>] - <日期>`——发布流程会用这个格式把对应内容抠出来当 Release 说明。

**带数据库迁移的版本要明确写出来**,回滚时用得着。

### 4. 提交并打 tag

```bash
git add -A
git commit -m "发布 $VERSION"
git tag "v$VERSION"
git push origin main "v$VERSION"
```

### 5. 等 CI

推 tag 会触发 `.github/workflows/release.yml`:

1. **prepare** —— 核对版本号一致,建一个草稿 Release,正文取自 CHANGELOG
2. **images** —— 构建并推送 `lab-inventory-backend` 和 `lab-inventory-frontend` 到 GHCR
3. **server-artifacts** —— Linux 后端二进制包
4. **web-artifacts** —— 前端静态包
5. **desktop** —— Windows / macOS(两种架构)/ Linux 安装包
6. **android** —— APK
7. **publish** —— 前面全过之后,把草稿 Release 转正

全流程大约 30–45 分钟,桌面端那几个任务最慢。

中途某个任务挂了,草稿 Release 会带着已经上传的产物停在那里。修好之后在 Actions 页面重跑失败的任务即可,不用删 tag——上传用的是 `--clobber`,重复上传会覆盖。

### 6. 验证

```bash
# 镜像能拉
docker pull ghcr.io/spinleft/lab-inventory-backend:0.2.0
docker pull ghcr.io/spinleft/lab-inventory-frontend:0.2.0

# 起一套干净的验证
mkdir /tmp/verify && cd /tmp/verify
curl -O https://raw.githubusercontent.com/spinleft/lab-inventory/v0.2.0/docker-compose.yml
curl -o .env https://raw.githubusercontent.com/spinleft/lab-inventory/v0.2.0/.env.example
# 填上必填项,COOKIE_SECURE=false
docker compose up -d
curl http://localhost:8080/api/v1/health_check
```

Release 页面上确认产物齐全:6 个桌面端文件、1 个 APK、1 个服务端包、1 个 web 包。

## 安卓签名

没配签名密钥时,CI 产出的是 `lab-inventory-<版本>-android-universal-unsigned.apk`——**这种包装不到手机上**,安卓会直接拒绝安装。配好之后产出的才是可安装的 `…-android-universal.apk`。

整个配置只需要做一次,之后每次发版自动生效。

### 1. 生成密钥库

在你自己的机器上(不是 CI 上)执行:

```bash
keytool -genkeypair -v \
  -keystore release.jks \
  -alias lab-inventory \
  -keyalg RSA -keysize 2048 \
  -validity 10000
```

`keytool` 随 JDK 一起装,装了 Android Studio 或任意 JDK 17 就有。

它会问几个问题:

- **口令**:自己定,记牢。密钥库口令和密钥口令可以设成同一个(直接回车表示沿用密钥库口令)
- **姓名/组织/城市/国家**:随便填,只写进证书里给用户看,不影响功能。可以填实验室或课题组的名字
- 最后确认时输入 `y`

`-validity 10000` 是 27 年。**别设短**:证书过期后就没法再发新版本了,而安卓不允许换密钥。

### 2. 转成 base64

GitHub secret 只能存文本,所以把二进制的密钥库编码一下:

```bash
# Linux / macOS / Git Bash
base64 -w0 release.jks > release.jks.base64
```

```powershell
# Windows PowerShell
[Convert]::ToBase64String([IO.File]::ReadAllBytes("release.jks")) | Set-Content release.jks.base64 -NoNewline
```

`-w0` / `-NoNewline` 是关键:不能有换行,否则 CI 那边解码会失败。

### 3. 添加 GitHub secret

仓库页面 → **Settings** → **Secrets and variables** → **Actions** → **New repository secret**,加四条:

| Secret                        | 值                                          |
| ----------------------------- | ------------------------------------------- |
| `ANDROID_KEYSTORE_BASE64`   | `release.jks.base64` 的**全部内容** |
| `ANDROID_KEYSTORE_PASSWORD` | 第 1 步设的密钥库口令                       |
| `ANDROID_KEY_ALIAS`         | `lab-inventory`(第 1 步 `-alias` 的值)  |
| `ANDROID_KEY_PASSWORD`      | 第 1 步设的密钥口令(同上就填一样的)         |

四条缺一不可。少配任何一条,CI 会在日志里给一条 warning 然后继续,产出未签名的包。

### 4. 保管好密钥库

把 `release.jks` 和两个口令存进密码管理器,或者离线备份。

**丢了就换不回来了。** 安卓用签名证书来认定"这是同一个应用的更新",换了密钥,已经装了旧版的用户升级时会报签名冲突,只能卸载重装(本地数据会丢)。

同时:`release.jks`、`release.jks.base64` **绝对不要提交进 git**。用完把本地的 base64 文件删掉。

### CI 是怎么签的

Gradle 那边没有配签名,所以 `tauri android build` 出来的一定是 `-unsigned` 的包。签名是构建之后单独做的:

```
zipalign -p -f 4  app-universal-release-unsigned.apk  → 对齐
apksigner sign --ks release.jks …                     → 签名,去掉 -unsigned 后缀
apksigner verify --print-certs                        → 验证并打印证书信息
```

之所以不走 Gradle 的 `signingConfigs`,是因为 `tauri android init` 生成的 `build.gradle.kts` 里根本没有签名配置,而 `gen/` 目录每次构建都重新生成——改了也留不住。

### 验证

发版跑完之后:

1. Release 页面上的 APK 文件名**不带** `-unsigned`
2. Actions 日志里 "签名 APK" 那一步打印了证书的 Signer 信息
3. 下载下来装到真机上试一次

也可以本地验证:

```bash
$ANDROID_HOME/build-tools/34.0.0/apksigner verify --print-certs lab-inventory-0.1.0-android-universal.apk
```

## 手动触发

补发或者重跑时,Actions 页面上手动运行"发布"工作流,填 tag 名(比如 `v0.2.0`)。

## 发版检查清单

- [ ] main 上 CI 全绿
- [ ] 四处版本号一致
- [ ] CHANGELOG 写了,标题格式对
- [ ] 有数据库迁移的话,CHANGELOG 里标注了
- [ ] tag 已推送
- [ ] 发布工作流全过,Release 已转正
- [ ] 镜像能拉,干净环境能起
- [ ] Release 产物齐全
