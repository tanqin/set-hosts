#!/usr/bin/env node
/**
 * 一键打包当前可用的所有端
 *
 *   npm run build:all                 # 桌面端 + Android（macOS 上再加 iOS）
 *   npm run build:all -- --check      # 只检查打包环境，不构建
 *   npm run build:all -- --skip-android
 *   npm run build:all -- --only android
 *
 * 各端相互独立：某一端失败不会中断其它端，最后统一汇报成败与产物位置。
 * 打包环境的检查也可以单独跑：`npm run build:check`。
 */
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

import {
  ROOT,
  capture,
  fail,
  findAndroidSdk,
  findJavaHome,
  findNdk,
  formatSize,
  isMac,
  isWindows,
  javaMajor,
  log,
  ok,
  readTauriConfig,
  warn,
} from './build-common.mjs';

/** Rust 侧 Android 交叉编译需要的 target */
const ANDROID_RUST_TARGETS = ['aarch64-linux-android', 'armv7-linux-androideabi', 'i686-linux-android', 'x86_64-linux-android'];

const TARGETS = [
  { key: 'desktop', name: '桌面端', script: 'build-desktop.mjs', outDir: 'dist-desktop' },
  { key: 'android', name: 'Android', script: 'build-android.mjs', outDir: 'dist-apk' },
  { key: 'ios', name: 'iOS', script: 'build-ios.mjs', outDir: 'dist-ios', onlyOnMac: true },
];

function printHelp() {
  console.log(`用法：npm run build:all [-- 参数]

      --check         只检查打包环境（等同 npm run build:check）
      --only <端>     只打某一端：desktop / android / ios
      --skip-desktop / --skip-android / --skip-ios
      --help          显示帮助

需要带值的参数时请直接运行脚本（Windows PowerShell 下 npm 会把 \`--flag value\` 里的
\`--flag\` 当成自己的参数吞掉）：
  node scripts/build-all.mjs --only android`);
}

function parseArgs(argv) {
  const options = { check: false, only: null, skip: new Set() };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--check') {
      options.check = true;
    } else if (arg === '--only') {
      options.only = String(argv[++i] ?? '');
    } else if (arg.startsWith('--skip-')) {
      options.skip.add(arg.slice('--skip-'.length));
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      fail(`未知参数：${arg}`);
      process.exit(1);
    }
  }

  if (options.only && !TARGETS.some((target) => target.key === options.only)) {
    fail(`--only 只支持：${TARGETS.map((target) => target.key).join(' / ')}`);
    process.exit(1);
  }
  return options;
}

/** 打包环境体检：把「为什么打不出包」在开工前就说清楚 */
function checkEnvironment() {
  const config = readTauriConfig();
  const javaHome = findJavaHome();
  const sdk = findAndroidSdk();
  const ndk = sdk ? findNdk(sdk) : null;
  const rustTargets = capture('rustup', ['target', 'list', '--installed']).split('\n').map((line) => line.trim());
  const missingRustTargets = ANDROID_RUST_TARGETS.filter((target) => !rustTargets.includes(target));

  console.log('');
  ok(`打包环境（Set Hosts v${config.version}）`);
  const line = (label, value) => console.log(`        ${label.padEnd(16)}${value}`);

  line('系统', `${process.platform} ${process.arch}`);
  line('Node', process.version);
  line('npm', capture('npm', ['-v']) || '未找到');
  line('tauri CLI', capture('npx', ['tauri', '--version']) || '未找到（先执行 npm install）');
  line('rustc', capture('rustc', ['--version']) || '未找到（需安装 Rust：https://rustup.rs）');
  line('cargo', capture('cargo', ['--version']) || '未找到');
  line('JDK 17+', javaHome ? `JDK ${javaMajor(javaHome)}：${javaHome}` : '未找到');
  line('Android SDK', sdk ?? '未找到');
  line('Android NDK', ndk ?? (sdk ? '未找到（SDK Manager 里安装）' : '—'));
  line(
    'Android target',
    rustTargets.length === 0
      ? '未找到 rustup（无法检查）'
      : missingRustTargets.length === 0
        ? '已安装'
        : `缺少 ${missingRustTargets.join(', ')}`,
  );

  const androidReady = Boolean(javaHome && sdk && ndk && (rustTargets.length === 0 || missingRustTargets.length === 0));

  console.log('');
  ok('可打包的端');
  line('桌面端', `可以（${isWindows ? 'Windows' : process.platform} 安装包）`);
  line('Android', androidReady ? '可以' : '缺依赖：请按上面的提示补齐 JDK / SDK / NDK / rustup target');
  line('iOS', isMac ? '可以（需已安装 Xcode）' : '不可以：iOS 只能在本机 macOS 上打包');
  if (!androidReady && missingRustTargets.length > 0) {
    console.log('');
    warn(`补齐 Android target：rustup target add ${missingRustTargets.join(' ')}`);
  }
  console.log('');
}

/** 依次执行各端脚本（某个失败不影响其它端） */
function buildTargets(options) {
  const startedAt = Date.now() - 2000;
  const results = [];

  for (const target of TARGETS) {
    if (options.only && options.only !== target.key) continue;
    if (options.skip.has(target.key)) {
      results.push({ target, status: 'skipped', reason: '按参数跳过' });
      continue;
    }
    if (target.onlyOnMac && !isMac) {
      results.push({ target, status: 'skipped', reason: '仅 macOS 支持' });
      continue;
    }

    console.log('');
    log(`=========== 开始打包：${target.name} ===========`);
    const result = spawnSync(process.execPath, [path.join(ROOT, 'scripts', target.script)], {
      cwd: ROOT,
      stdio: 'inherit',
    });
    results.push({ target, status: result.status === 0 ? 'ok' : 'failed', reason: '' });
  }

  // 汇总：各端的产物目录里，本次构建新出现的文件
  console.log('');
  ok('=========== 打包结果 ===========');
  let failed = 0;
  for (const { target, status, reason } of results) {
    const mark = { ok: '成功', failed: '失败', skipped: '跳过' }[status];
    const suffix = reason ? `（${reason}）` : '';
    console.log(`        ${target.name.padEnd(10)}${mark}${suffix}`);

    if (status === 'failed') {
      failed += 1;
      continue;
    }
    if (status !== 'ok') continue;

    const outDir = path.join(ROOT, target.outDir);
    if (!fs.existsSync(outDir)) continue;
    const files = fs
      .readdirSync(outDir, { withFileTypes: true })
      .filter((entry) => entry.isFile() && fs.statSync(path.join(outDir, entry.name)).mtimeMs >= startedAt)
      .map((entry) => path.join(outDir, entry.name))
      .sort();
    for (const file of files) {
      console.log(`        └─ ${target.outDir}/${path.basename(file)}  ${formatSize(fs.statSync(file).size)}`);
    }
  }
  console.log('');

  if (failed > 0) {
    fail(`${failed} 个端打包失败，详见上方日志`);
    process.exit(1);
  }
  ok('全部完成');
}

const options = parseArgs(process.argv.slice(2));
if (options.check) {
  checkEnvironment();
} else {
  buildTargets(options);
}
