#!/usr/bin/env node
/**
 * 打包桌面端安装包
 *
 *   npm run build:desktop            # 打当前系统的安装包
 *   npm run build:desktop -- --debug # 调试版（带 devtools，体积大）
 *   npm run build:windows | build:macos | build:linux
 *                                    # 显式指定系统：本机系统不匹配时会直接报错，
 *                                    # 而不是抛一堆看不懂的原生工具链错误
 *
 * Tauri 不支持跨平台打包：Windows 上只能出 .exe/.msi，macOS 上出 .dmg，
 * Linux 上出 .deb/.rpm/.AppImage，各自要在对应系统上运行。
 *
 * 产物统一收集到 dist-desktop/（可用 --out 改目录）。
 */
import fs from 'node:fs';
import path from 'node:path';

import {
  ROOT,
  abort,
  collectNewFiles,
  normalizeArtifactName,
  isLinux,
  isMac,
  isWindows,
  log,
  printArtifacts,
  publish,
  readTauriConfig,
  run,
} from './build-common.mjs';

/** 各系统的安装包扩展名 */
const BUNDLE_EXTENSIONS = {
  windows: ['.exe', '.msi'],
  macos: ['.dmg', '.app.tar.gz'],
  linux: ['.deb', '.rpm', '.appimage'],
};

const hostPlatform = isWindows ? 'windows' : isMac ? 'macos' : isLinux ? 'linux' : process.platform;
const hostPlatformName = { windows: 'Windows', macos: 'macOS', linux: 'Linux' }[hostPlatform] ?? process.platform;

function printHelp() {
  console.log(`用法：npm run build:desktop [-- 参数]

      --platform <系统>  要求当前系统是 windows / macos / linux，否则直接报错
      --out <目录>       产物收集目录（默认 dist-desktop）
      --debug            打调试版
      --help             显示帮助

（其余参数会原样透传给 \`tauri build\`，例如 --bundles nsis --target x86_64-pc-windows-msvc。
Windows PowerShell 下 npm 会吞掉 \`--flag value\` 里的 \`--flag\`，带参请直接运行脚本：
  node scripts/build-desktop.mjs --bundles nsis）`);
}

function parseArgs(argv) {
  const options = { platform: null, out: 'dist-desktop', debug: false, passthrough: [] };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--platform') {
      options.platform = String(argv[++i] ?? '');
    } else if (arg === '--out') {
      options.out = String(argv[++i] ?? options.out);
    } else if (arg === '--debug') {
      options.debug = true;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      options.passthrough.push(arg);
    }
  }

  if (options.platform && !['windows', 'macos', 'linux'].includes(options.platform)) {
    abort(`--platform 只支持 windows / macos / linux，收到：${options.platform}`);
  }
  return options;
}

/** 候选产物目录：target/<variant>/bundle 以及指定 target triple 时的 target/<triple>/<variant>/bundle */
function bundleDirs(variant) {
  const targetDir = path.join(ROOT, 'src-tauri', 'target');
  if (!fs.existsSync(targetDir)) return [];

  const dirs = [path.join(targetDir, variant, 'bundle')];
  for (const entry of fs.readdirSync(targetDir, { withFileTypes: true })) {
    if (entry.isDirectory()) dirs.push(path.join(targetDir, entry.name, variant, 'bundle'));
  }
  return dirs.filter((dir) => fs.existsSync(dir));
}

const options = parseArgs(process.argv.slice(2));
const config = readTauriConfig();

if (options.platform && options.platform !== hostPlatform) {
  const wanted = { windows: 'Windows', macos: 'macOS', linux: 'Linux' }[options.platform];
  abort(
    `当前系统是 ${hostPlatformName}，打不了 ${wanted} 的安装包`,
    'Tauri 不支持跨平台打包：请到对应系统的机器（或 CI）上执行这条命令',
    hostPlatform === 'windows' ? '本机可以打包：npm run build:desktop 与 npm run build:android' : '',
  );
}

const variant = options.debug ? 'debug' : 'release';
log(`打包桌面端安装包（v${config.version}，${hostPlatformName}，${variant}）`);

const startedAt = Date.now() - 2000;
const args = ['tauri', 'build', ...options.passthrough];
if (options.debug) args.push('--debug');

if (!run('npx', args)) {
  abort('桌面端打包失败：请看上方 Cargo / 打包工具的输出');
}

const extensions = BUNDLE_EXTENSIONS[hostPlatform] ?? [];
const files = bundleDirs(variant).flatMap((dir) => collectNewFiles(dir, extensions, startedAt));

if (files.length === 0) {
  abort(
    '没有找到新生成的安装包',
    `检查 src-tauri/target/${variant}/bundle 目录；若 Cargo 增量构建没触发打包，可先删除该 bundle 目录再重试`,
  );
}

const outDir = path.resolve(ROOT, options.out);
const artifacts = [];
for (const file of files) {
  // Tauri 各平台的原始命名规则不一致（Linux 还是小写的 set-hosts_*），
  // 统一重写为 `Set Hosts_<版本>_<架构/变体>.<扩展名>`
  const name = normalizeArtifactName(path.basename(file), {
    product: config.productName,
    version: config.version,
  });
  artifacts.push(publish(file, outDir, name));
  log(`${path.basename(file)} → ${options.out}/${name}`);
}

printArtifacts(`桌面端产物（v${config.version}）：`, artifacts);
