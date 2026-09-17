#!/usr/bin/env node
/**
 * 打包 Android（APK / AAB）
 *
 *   npm run build:android                      # 默认 arm64（主流真机 ABI）
 *   npm run build:android -- --all             # 一次打包全部 ABI（通用包，体积最大）
 *   npm run build:android -- --split-per-abi   # 每个 ABI 一个包（体积小，适合分发）
 *   npm run build:android -- --targets aarch64,armv7
 *   npm run build:android -- --aab             # 只出上架 Google Play 用的 AAB
 *   npm run build:android -- --debug           # 调试包（不混淆，便于看日志）
 *
 * 环境要求：JDK 17+ 与 Android SDK（含 NDK）。脚本会自动探测，也可以显式指定：
 *   npm run build:android -- --java-home "C:/Program Files/Android/Android Studio/jbr"
 *   npm run build:android -- --sdk "D:/Software/Android/Sdk"
 *
 * 产物统一收集到 dist-apk/（可用 --out 改目录）。
 */
import fs from 'node:fs';
import path from 'node:path';

import {
  ROOT,
  abort,
  androidEnv,
  collectNewFiles,
  fail,
  findAndroidSdk,
  findJavaHome,
  findNdk,
  log,
  printArtifacts,
  publish,
  readTauriConfig,
  run,
  warn,
} from './build-common.mjs';

const OUTPUTS_DIR = path.join(ROOT, 'src-tauri', 'gen', 'android', 'app', 'build', 'outputs');

/** Tauri 的 ABI 目录名 → 产物文件名里用的标签（arm64-v8a 太啰嗦） */
const ABI_LABELS = {
  'arm64-v8a': 'arm64',
  'armeabi-v7a': 'armv7',
  x86: 'x86',
  x86_64: 'x86_64',
  // --targets 里写的是 Rust 侧的名字
  aarch64: 'arm64',
  armv7: 'armv7',
  i686: 'x86',
};

/** Rust 侧 ABI 名 → Gradle 侧的 ABI 目录名 */
const ABI_DIRS = {
  aarch64: 'arm64-v8a',
  armv7: 'armeabi-v7a',
  i686: 'x86',
  x86_64: 'x86_64',
};

function printHelp() {
  console.log(`用法：npm run build:android [-- 参数]

  -t, --targets <列表>   ABI，逗号分隔（默认 aarch64）
                         可选：aarch64 / armv7 / x86_64 / i686
      --all              等价于 --targets aarch64,armv7,x86_64,i686
      --split-per-abi    按 ABI 分别出包
      --apk              出 APK（默认）
      --aab              出 AAB（Google Play 上架用）
      --debug            调试包
      --out <目录>       产物收集目录（默认 dist-apk）
      --sdk <目录>       Android SDK 目录
      --java-home <目录> JDK 17+ 目录

需要带值的参数时请直接运行脚本（Windows PowerShell 下 npm 会把 \`--flag value\` 里的
\`--flag\` 当成自己的参数吞掉）：
  node scripts/build-android.mjs --targets aarch64,armv7 --split-per-abi`);
}

function parseArgs(argv) {
  const options = {
    targets: ['aarch64'],
    bundle: 'apk',
    split: false,
    debug: false,
    out: 'dist-apk',
    sdk: null,
    javaHome: null,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--all') {
      options.targets = ['aarch64', 'armv7', 'x86_64', 'i686'];
    } else if (arg === '--targets' || arg === '-t') {
      options.targets = String(argv[++i] ?? '')
        .split(',')
        .map((item) => item.trim())
        .filter(Boolean);
    } else if (arg === '--split-per-abi') {
      options.split = true;
    } else if (arg === '--apk') {
      options.bundle = 'apk';
    } else if (arg === '--aab') {
      options.bundle = 'aab';
    } else if (arg === '--debug') {
      options.debug = true;
    } else if (arg === '--out') {
      options.out = String(argv[++i] ?? options.out);
    } else if (arg === '--sdk') {
      options.sdk = argv[++i] ?? null;
    } else if (arg === '--java-home') {
      options.javaHome = argv[++i] ?? null;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      abort(`未知参数：${arg}`, '用 `npm run build:android -- --help` 查看可用参数');
    }
  }

  if (options.targets.length === 0) abort('--targets 不能为空');
  for (const target of options.targets) {
    if (!ABI_LABELS[target]) abort(`不支持的 ABI：${target}`, '可选：aarch64 / armv7 / x86_64 / i686');
  }
  return options;
}

/** 从产物路径推断 ABI 段：apk/<abi>/<variant>/… 或 bundle/<abi><Variant>/… */
function artifactLabel(file, { singleTarget, targets }) {
  const relative = path.relative(OUTPUTS_DIR, file);
  const dir = relative.split(path.sep)[1] ?? '';
  const abi = dir.replace(/(Release|Debug)$/, '');

  if (abi === 'universal') {
    // 单 ABI 且未拆分时 Tauri 也放进 universal 目录，此时包内只有那一个 ABI
    return singleTarget ? (ABI_LABELS[targets[0]] ?? 'universal') : 'universal';
  }
  return ABI_LABELS[abi] || abi || 'universal';
}

/**
 * 按 Tauri 的产物命名规则推算路径，只返回真实存在的文件
 *
 * 不用「比构建开始时间新」来判断：源码没变时 Gradle 判定 up-to-date，产物文件
 * 不会被重写，靠时间戳会误判成「没有生成产物」。
 */
function expectedArtifacts(targets) {
  const variant = options.debug ? 'debug' : 'release';
  const root = path.join(OUTPUTS_DIR, options.bundle === 'aab' ? 'bundle' : 'apk');

  if (options.bundle === 'aab') {
    // bundle/universalRelease/app-universal-release.aab
    return [path.join(root, `universal${options.debug ? 'Debug' : 'Release'}`, `app-universal-${variant}.aab`)].filter(
      (file) => fs.existsSync(file),
    );
  }

  if (options.split) {
    // apk/<abi>/<variant>/app-<abi>-<variant>.apk
    return targets
      .map((target) => {
        const abiDir = ABI_DIRS[target];
        return path.join(root, abiDir, variant, `app-${abiDir}-${variant}.apk`);
      })
      .filter((file) => fs.existsSync(file));
  }

  // 未拆分：所有 ABI 打进同一个包，Tauri 放在 universal 目录
  return [path.join(root, 'universal', variant, `app-universal-${variant}.apk`)].filter((file) =>
    fs.existsSync(file),
  );
}

/** 兜底：产物目录命名规则若与预期不同，取目录里时间最新的匹配文件 */
function newestArtifacts(dir, ext, keepAll) {
  const files = collectNewFiles(dir, [ext], 0).sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs);
  if (keepAll) return files;
  return files.length > 0 ? [files[0]] : [];
}

const options = parseArgs(process.argv.slice(2));
const config = readTauriConfig();

const javaHome = options.javaHome ?? findJavaHome();
if (!javaHome) {
  abort(
    '未找到 JDK 17+：Android 打包必须有 JDK',
    '安装 JDK 17（Temurin / Zulu / Android Studio 自带的 jbr 均可）后设置 JAVA_HOME',
    '或用 --java-home <目录> 指定，例如：npm run build:android -- --java-home "C:/Program Files/Android/Android Studio/jbr"',
  );
}

const sdk = options.sdk ?? findAndroidSdk();
if (!sdk) {
  abort(
    '未找到 Android SDK',
    '用 Android Studio → SDK Manager 安装，并设置 ANDROID_HOME 指向 SDK 目录',
    '或用 --sdk <目录> 指定，例如：npm run build:android -- --sdk "D:/Software/Android/Sdk"',
  );
}

const ndk = findNdk(sdk);
if (!ndk) {
  warn(`Android SDK 里没有 NDK（${path.join(sdk, 'ndk')} 为空）：Rust 交叉编译会失败，请在 SDK Manager 里安装 NDK`);
}

const bundleName = options.bundle === 'aab' ? 'AAB' : 'APK';
log(`打包 Android ${bundleName}（v${config.version}）`);
log(`JDK：${javaHome}`);
log(`SDK：${sdk}${ndk ? `（NDK ${path.basename(ndk)}）` : ''}`);
log(`ABI：${options.targets.join(', ')}${options.split ? '（按 ABI 拆分）' : ''}`);

// 容忍文件系统时间戳精度：产物用「比构建开始时间新」来判断
const startedAt = Date.now() - 2000;

const args = [
  'tauri',
  'android',
  'build',
  options.bundle === 'aab' ? '--aab' : '--apk',
  '-t',
  ...options.targets,
  '--ci',
];
if (options.split) args.push('--split-per-abi');
if (options.debug) args.push('--debug');

if (!run('npx', args, { env: androidEnv(javaHome, sdk, ndk) })) {
  fail('Android 打包失败：请看上方 Gradle / Cargo 的输出');
  process.exit(1);
}

const variant = options.debug ? 'debug' : 'release';
const searchDir = path.join(OUTPUTS_DIR, options.bundle === 'aab' ? 'bundle' : 'apk');

let files = expectedArtifacts(options.targets);
if (files.length === 0) {
  warn(`按预期路径没找到 ${bundleName}，改为扫描 ${path.relative(ROOT, searchDir)}`);
  files = newestArtifacts(searchDir, `.${options.bundle}`, options.split || options.targets.length > 1);
}
if (files.length === 0) {
  abort(
    `没有找到 ${bundleName}（${path.relative(ROOT, searchDir)}）`,
    '构建可能被 Gradle 缓存跳过且目录里没有历史产物，可先删除 src-tauri/gen/android/app/build 再重试',
  );
}
if (!files.some((file) => fs.statSync(file).mtimeMs >= startedAt)) {
  warn(`Gradle 判定无需重新打包（源码未变），复用上次生成的 ${bundleName}`);
}

const singleTarget = !options.split && options.targets.length === 1;
const outDir = path.resolve(ROOT, options.out);
const artifacts = [];

for (const file of files) {
  const label = artifactLabel(file, { singleTarget, targets: options.targets });
  const name = `set-hosts-${label}-${variant}.${options.bundle}`;
  artifacts.push(publish(file, outDir, name));
  log(`${path.basename(file)} → ${options.out}/${name}`);
}

printArtifacts(`Android 产物（v${config.version}）：`, artifacts);
