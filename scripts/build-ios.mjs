#!/usr/bin/env node
/**
 * 打包 iOS
 *
 *   npm run build:ios
 *
 * Tauri 的 iOS 打包依赖 Xcode，只能在 macOS 上执行；Windows / Linux 上会直接提示，
 * 不会抛出一堆看不懂的工具链错误。产物（.ipa）收集到 dist-ios/。
 */
import path from 'node:path';

import {
  ROOT,
  abort,
  collectNewFiles,
  isMac,
  log,
  printArtifacts,
  publish,
  readTauriConfig,
  run,
  warn,
} from './build-common.mjs';

const options = process.argv.slice(2);
if (options.includes('--help') || options.includes('-h')) {
  console.log('用法：npm run build:ios\n\n只能在 macOS（含 Xcode 与 iOS SDK）上运行。');
  process.exit(0);
}

if (!isMac) {
  abort(
    `当前系统是 ${process.platform}，打不了 iOS 包`,
    'iOS 打包需要 Xcode，只能在 macOS 上执行',
    '本机可以打包：npm run build:desktop 与 npm run build:android',
  );
}

const config = readTauriConfig();
const startedAt = Date.now() - 2000;

log(`打包 iOS（v${config.version}）`);
if (!run('npx', ['tauri', 'ios', 'build', ...options])) {
  abort('iOS 打包失败：请看上方 Xcode / Cargo 的输出');
}

const files = collectNewFiles(path.join(ROOT, 'src-tauri', 'gen', 'apple'), ['.ipa'], startedAt);
if (files.length === 0) {
  warn('没有找到 .ipa：Tauri 打包出的 .app 位于 src-tauri/gen/apple/build 下，');
  warn('需要在 Xcode（Window → Organizer → Distribute App）里导出 .ipa');
  process.exit(0);
}

const artifacts = [];
for (const file of files) {
  artifacts.push(publish(file, path.join(ROOT, 'dist-ios'), path.basename(file)));
  log(`${path.basename(file)} → dist-ios/${path.basename(file)}`);
}

printArtifacts(`iOS 产物（v${config.version}）：`, artifacts);
