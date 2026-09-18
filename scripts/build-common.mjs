/**
 * 打包脚本的公共工具：定位项目根目录、探测构建环境（JDK / Android SDK / NDK）、
 * 调用 tauri CLI、收集构建产物。
 *
 * 这些脚本由 package.json 的 scripts 调用，也可以直接 `node scripts/xxx.mjs` 运行。
 */
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

/** 项目根目录（scripts/ 的上一级） */
export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export const isWindows = process.platform === 'win32';
export const isMac = process.platform === 'darwin';
export const isLinux = process.platform === 'linux';

const paint = (code, text) => (process.stdout.isTTY ? `\x1b[${code}m${text}\x1b[0m` : text);
export const log = (msg) => console.log(`${paint(36, '[build]')} ${msg}`);
export const warn = (msg) => console.log(`${paint(33, '[build]')} ${msg}`);
export const fail = (msg) => console.error(`${paint(31, '[build]')} ${msg}`);
export const ok = (msg) => console.log(`${paint(32, '[build]')} ${msg}`);

/** 报错并给出可执行的建议，然后退出 */
export function abort(message, ...hints) {
  fail(message);
  for (const hint of hints.filter(Boolean)) fail(`        ${hint}`);
  process.exit(1);
}

/** 读取 tauri 配置（版本号、productName 都从这里取，保证与打包产物一致） */
export function readTauriConfig() {
  return JSON.parse(fs.readFileSync(path.join(ROOT, 'src-tauri', 'tauri.conf.json'), 'utf8'));
}

export function formatSize(bytes) {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

/** 运行命令并把输出直接透传到终端；返回是否成功 */
export function run(command, args = [], { env, cwd = ROOT } = {}) {
  log(`$ ${[command, ...args].join(' ')}`);
  const result = spawnSync(command, args, {
    cwd,
    env: { ...process.env, ...env },
    stdio: 'inherit',
    // Windows 上必须走 shell 才能解析 npx.cmd / gradlew.bat
    shell: isWindows,
  });
  if (result.error) {
    fail(`无法执行 ${command}：${result.error.message}`);
    return false;
  }
  return result.status === 0;
}

/** 运行命令并捕获输出（用于探测版本号等），失败时返回空串 */
export function capture(command, args = []) {
  const result = spawnSync(command, args, { cwd: ROOT, encoding: 'utf8', shell: isWindows });
  return result.status === 0 ? (result.stdout ?? '').trim() : '';
}

// ---------------- JDK 探测 ----------------

/** 读取 JDK 主版本号；找不到 release 文件时返回 null */
export function javaMajor(javaHome) {
  const release = path.join(javaHome, 'release');
  if (!fs.existsSync(release)) return null;
  // JAVA_VERSION 可能是 "17.0.5" 或 "1.8.0_221"
  const match = /JAVA_VERSION="(?:1\.)?(\d+)/.exec(fs.readFileSync(release, 'utf8'));
  return match ? Number(match[1]) : null;
}

const isJdkRoot = (dir) => fs.existsSync(path.join(dir, 'bin', isWindows ? 'java.exe' : 'java'));

/** 展开一个「可能放着若干 JDK」的目录：jdk-17.0.5、jbr、xxx.jdk/Contents/Home 等 */
function expandJavaRoots(root) {
  if (!root || !fs.existsSync(root)) return [];
  if (isJdkRoot(root)) return [root];

  const found = [];
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const child = path.join(root, entry.name);
    if (isJdkRoot(child)) {
      found.push(child);
    } else if (entry.name === 'Contents') {
      // macOS：xxx.jdk/Contents/Home
      const home = path.join(child, 'Home');
      if (isJdkRoot(home)) found.push(home);
    }
  }
  return found;
}

/** 常见 JDK 安装位置（Android Studio 自带的 jbr 也可以直接用来打 Android 包） */
const JAVA_ROOT_HINTS = [
  'C:/Program Files/Android/Android Studio/jbr',
  'C:/Program Files/Java',
  'C:/Program Files/Eclipse Adoptium',
  'C:/Program Files/Microsoft',
  'C:/Program Files/Zulu',
  'C:/Program Files/BellSoft',
  'C:/Java',
  'C:/Android/jbr',
  'D:/Software/Java',
  'D:/Java',
  'D:/Program Files/Java',
  'D:/Program Files/Android/Android Studio/jbr',
  'E:/Software/Java',
  '/usr/lib/jvm',
  '/Library/Java/JavaVirtualMachines',
  '/opt/homebrew/opt',
];

/**
 * 找到可用于 Android 打包的 JDK（Tauri 要求 JDK 17+）
 *
 * 优先用 17：与已验证可用的构建环境保持一致；没有 17 时取版本最高的一个。
 */
export function findJavaHome() {
  const candidates = [process.env.JAVA_HOME, ...JAVA_ROOT_HINTS].flatMap(expandJavaRoots);

  const usable = candidates
    .map((home) => ({ home, major: javaMajor(home) }))
    .filter((item) => item.major !== null && item.major >= 17);
  if (usable.length === 0) return null;

  const preferred = usable.filter((item) => item.major === 17);
  const pool = preferred.length > 0 ? preferred : usable;
  pool.sort((a, b) => b.major - a.major);
  return pool[0].home;
}

// ---------------- Android SDK / NDK 探测 ----------------

const ANDROID_SDK_HINTS = [
  process.env.ANDROID_HOME,
  process.env.ANDROID_SDK_ROOT,
  process.env.ANDROID_SDK,
  process.env.LOCALAPPDATA && path.join(process.env.LOCALAPPDATA, 'Android', 'Sdk'),
  process.env.USERPROFILE && path.join(process.env.USERPROFILE, 'AppData', 'Local', 'Android', 'Sdk'),
  process.env.HOME && path.join(process.env.HOME, 'Android', 'Sdk'),
  process.env.HOME && path.join(process.env.HOME, 'Library', 'Android', 'sdk'),
  'C:/Android/Sdk',
  'C:/Android/sdk',
  'D:/Android/Sdk',
  'D:/Android/sdk',
  'D:/Software/Android/Sdk',
  'E:/Android/Sdk',
];

const isAndroidSdk = (dir) =>
  Boolean(dir) && fs.existsSync(path.join(dir, 'platform-tools')) && fs.existsSync(path.join(dir, 'platforms'));

export function findAndroidSdk() {
  for (const dir of ANDROID_SDK_HINTS) {
    if (isAndroidSdk(dir)) return dir;
  }
  return null;
}

/** SDK 下版本最高的 NDK（Rust 交叉编译 Android 必须要有） */
export function findNdk(sdk) {
  const ndkRoot = path.join(sdk, 'ndk');
  if (!fs.existsSync(ndkRoot)) return null;
  const versions = fs
    .readdirSync(ndkRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
  return versions.length > 0 ? path.join(ndkRoot, versions.at(-1)) : null;
}

/** Android 打包所需的环境变量（JDK 优先，避免 Gradle 用到系统里其它版本的 Java） */
export function androidEnv(javaHome, sdk, ndk) {
  return {
    JAVA_HOME: javaHome,
    ANDROID_HOME: sdk,
    ANDROID_SDK_ROOT: sdk,
    ...(ndk ? { ANDROID_NDK_HOME: ndk } : {}),
    PATH: `${path.join(javaHome, 'bin')}${path.delimiter}${process.env.PATH ?? ''}`,
  };
}

// ---------------- 产物收集 ----------------

/**
 * 递归找出 `since` 之后新生成的、扩展名匹配的文件
 *
 * 用「列出新文件」而不是写死路径：Tauri 不同版本的产物目录命名有差异
 * （如 apk/universal/release/ 与 bundle/universalRelease/）。
 */
export function collectNewFiles(dir, extensions, since) {
  const found = [];
  if (!fs.existsSync(dir)) return found;

  const walk = (current) => {
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) {
        walk(full);
        continue;
      }
      const lower = entry.name.toLowerCase();
      if (!extensions.some((ext) => lower.endsWith(ext))) continue;
      if (fs.statSync(full).mtimeMs >= since) found.push(full);
    }
  };
  walk(dir);

  found.sort();
  return found;
}

/** 正则转义：产品名里可能有空格等字符 */
const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** 多段扩展名（Tauri 的 macOS .app.tar.gz），需优先于单点扩展名识别 */
const MULTI_PART_EXTENSIONS = ['app.tar.gz'];

/** 标准语言标签（en-US / zh-CN 等）：连字符是标签的一部分，不能替换成下划线 */
const LOCALE_TAG = /^[a-z]{2}-[A-Z]{2}$/;

/**
 * 统一产物命名：<产品名>_<版本>[_<片段>…].<扩展名>
 *
 * 例：`Set Hosts_0.1.2_x64_setup.exe`、`Set Hosts_0.1.2_arm64_release.apk`
 */
export function buildArtifactName({ product, version, segments = [], ext }) {
  // 语言标签（en-US）保留连字符，其余片段统一用下划线
  const tail = segments
    .filter(Boolean)
    .map((seg) => (LOCALE_TAG.test(seg) ? seg : String(seg).replace(/[-\s]+/g, '_')))
    .join('_');
  const name = tail ? `${product}_${version}_${tail}` : `${product}_${version}`;
  return ext ? `${name}.${ext}` : name;
}

/**
 * 把 Tauri 原始产物名重写为统一命名。
 *
 * 各平台原始规则并不一致（Windows `产品_版本_x64-setup.exe`、Linux deb `set-hosts_版本_amd64.deb`、
 * rpm `set-hosts-版本-1.x86_64.rpm`），这里统一成 `<产品名>_<版本>_<架构/变体>.<扩展名>`。
 */
export function normalizeArtifactName(originalName, { product, version }) {
  const lower = originalName.toLowerCase();

  // 先识别多段扩展名，再退回最后一段
  let ext = '';
  let stem = originalName;
  for (const multi of MULTI_PART_EXTENSIONS) {
    if (lower.endsWith(`.${multi}`)) {
      ext = multi;
      stem = originalName.slice(0, -(multi.length + 1));
      break;
    }
  }
  if (!ext) {
    const dot = stem.lastIndexOf('.');
    if (dot > 0) {
      ext = stem.slice(dot + 1);
      stem = stem.slice(0, dot);
    }
  }

  // 去掉产品名（含 deb/rpm 里的小写 set-hosts 形式）与版本号，剩下的就是架构 / 变体片段
  const productVariants = [product, product.replace(/\s+/g, '-')];
  let tail = stem;
  for (const variant of productVariants) {
    tail = tail.replace(new RegExp(escapeRegExp(variant), 'gi'), '');
  }
  tail = tail.split(version).join('');

  tail = tail
    .split('_')
    // 语言标签保留标准写法（en-US / zh-CN），其余片段的连字符 / 点号统一为下划线
    .map((seg) => (LOCALE_TAG.test(seg) ? seg : seg.replace(/[-.\s]+/g, '_')))
    .join('_')
    .replace(/^_+|_+$/g, '') // 去掉首尾多余分隔符
    .replace(/_{2,}/g, '_')
    .replace(/^\d+_/, ''); // rpm 的 release 号（set-hosts-0.1.2-1.x86_64.rpm）

  // 按片段传，让语言标签在 buildArtifactName 里也能被识别保留
  return buildArtifactName({ product, version, segments: tail.split('_'), ext });
}

/** 复制产物到发布目录（同名文件直接覆盖），返回目标路径 */
export function publish(from, outDir, name) {  fs.mkdirSync(outDir, { recursive: true });
  const to = path.join(outDir, name);
  fs.copyFileSync(from, to);
  // copyFileSync 沿用源文件的修改时间；产物没被重新打包时源文件是旧的
  // （Gradle up-to-date），显式刷新时间戳，汇总「本次产物」时才不会漏掉
  const now = new Date();
  fs.utimesSync(to, now, now);
  return to;
}

/** 打印产物清单 */
export function printArtifacts(title, artifacts) {
  console.log('');
  ok(title);
  for (const file of artifacts) {
    if (fs.existsSync(file)) {
      console.log(`        ${path.relative(ROOT, file).split(path.sep).join('/')}  ${formatSize(fs.statSync(file).size)}`);
    }
  }
  console.log('');
}
