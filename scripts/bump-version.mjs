#!/usr/bin/env node
/**
 * 统一版本号更新脚本
 *
 * 用法：
 *   node scripts/bump-version.mjs patch|minor|major   # 递增版本、同步所有文件、提交并打 v* 标签
 *   node scripts/bump-version.mjs                     # 不递增，仅同步所有文件为 package.json 当前版本
 *
 * 可选参数：
 *   --check       只校验各处版本号是否一致，不做任何修改
 *   --no-commit   不提交，只打标签
 *   --no-git      完全跳过 git 操作（不提交、不打标签）
 *   --force-tag   标签已存在时先删除再重建
 *
 * 同步目标（任何一处漏改都会导致打包后版本不一致）：
 *   1. package.json                                        version
 *   2. package-lock.json                                   version + packages[""].version
 *   3. src-tauri/tauri.conf.json                           version（桌面端安装包 / 运行时 getVersion）
 *   4. src-tauri/Cargo.toml                                version
 *   5. src-tauri/Cargo.lock                                set-hosts 包版本
 *   6. src-tauri/gen/android/app/tauri.properties          versionName / versionCode（APK / AAB）
 *   7. src-tauri/gen/android/app/src/main/assets/tauri.conf.json（Android 打包时内嵌的配置副本）
 *   8. src-tauri/gen/apple/**\/project.pbxproj             MARKETING_VERSION（iOS，存在时才同步）
 */

import { readFileSync, writeFileSync, existsSync, readdirSync, statSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, join, resolve } from 'node:path'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const p = (...parts) => join(root, ...parts)

const SEMVER = /^(\d+)\.(\d+)\.(\d+)$/

function readJson(file) {
  return JSON.parse(readFileSync(file, 'utf-8'))
}

function writeJson(file, data) {
  writeFileSync(file, JSON.stringify(data, null, 2) + '\n', 'utf-8')
}

/** 替换文件中匹配到的第一段文本；返回是否发生替换 */
function replaceOnce(file, regex, replacement) {
  if (!existsSync(file)) return false
  const src = readFileSync(file, 'utf-8')
  if (!regex.test(src)) return false
  writeFileSync(file, src.replace(regex, replacement), 'utf-8')
  return true
}

function bump(version, type) {
  const m = version.match(SEMVER)
  if (!m) throw new Error(`版本号格式不是 x.y.z：${version}`)
  let [major, minor, patch] = m.slice(1).map(Number)
  if (type === 'major') {
    major += 1
    minor = 0
    patch = 0
  } else if (type === 'minor') {
    minor += 1
    patch = 0
  } else {
    patch += 1
  }
  return `${major}.${minor}.${patch}`
}

/** Android 要求 versionCode 单调递增，因此新值必须大于旧值；版本未变则保持原值 */
function nextVersionCode(oldCode, oldName, version) {
  if (oldName === version) return oldCode
  const [major, minor, patch] = version.split('.').map(Number)
  const computed = major * 1_000_000 + minor * 10_000 + patch * 100
  return computed > oldCode ? computed : oldCode + 1
}

/** 在仓库根目录执行 git 命令 */
function git(...gitArgs) {
  return spawnSync('git', gitArgs, { cwd: root, encoding: 'utf-8' })
}

/** 递归查找指定文件名的所有路径 */
function findFiles(dir, filename) {
  if (!existsSync(dir)) return []
  const out = []
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry)
    if (statSync(full).isDirectory()) out.push(...findFiles(full, filename))
    else if (entry === filename) out.push(full)
  }
  return out
}

const args = process.argv.slice(2)
const FLAGS = ['--check', '--no-git', '--no-commit', '--force-tag']
const checkOnly = args.includes('--check')
const noGit = args.includes('--no-git')
const noCommit = args.includes('--no-commit')
const forceTag = args.includes('--force-tag')
const type = args.find((a) => ['patch', 'minor', 'major'].includes(a))
if (args.some((a) => !FLAGS.includes(a) && !['patch', 'minor', 'major'].includes(a))) {
  console.error(
    '用法: node scripts/bump-version.mjs [patch|minor|major] [--check] [--no-git] [--no-commit] [--force-tag]',
  )
  process.exit(1)
}

/** 收集各文件中的版本号，用于 --check 校验一致性 */
function collectVersions() {
  const readIf = (file) => (existsSync(file) ? readFileSync(file, 'utf-8') : '')
  const pick = (src, regex) => src.match(regex)?.[1] ?? null
  const out = []
  const push = (label, value) => value && out.push({ label, value })

  push('package.json', readJson(p('package.json')).version)
  const lock = readJson(p('package-lock.json'))
  push('package-lock.json', lock.version)
  push('package-lock.json packages[""]', lock.packages?.['']?.version)
  push('src-tauri/tauri.conf.json', pick(readIf(p('src-tauri', 'tauri.conf.json')), /"version"\s*:\s*"([^"]*)"/))
  push('src-tauri/Cargo.toml', pick(readIf(p('src-tauri', 'Cargo.toml')), /^version\s*=\s*"([^"]*)"/m))
  push('src-tauri/Cargo.lock', pick(readIf(p('src-tauri', 'Cargo.lock')), /name = "set-hosts"\nversion = "([^"]*)"/))
  push('android/tauri.properties', pick(readIf(p('src-tauri', 'gen', 'android', 'app', 'tauri.properties')), /versionName\s*=\s*(\S*)/))
  push(
    'android/assets/tauri.conf.json',
    pick(
      readIf(p('src-tauri', 'gen', 'android', 'app', 'src', 'main', 'assets', 'tauri.conf.json')),
      /"version"\s*:\s*"([^"]*)"/,
    ),
  )
  return out
}

if (checkOnly) {
  const versions = collectVersions()
  const base = versions[0]?.value
  const mismatched = versions.filter((v) => v.value !== base)
  if (mismatched.length) {
    console.error(`\n版本号不一致（基准 package.json = ${base}）：`)
    for (const v of mismatched) console.error(`  ${v.label.padEnd(32)} ${v.value}`)
    console.error('\n请运行 npm run version:sync 修复\n')
    process.exit(1)
  }
  console.log(`\n版本一致：所有 ${versions.length} 处均为 ${base}\n`)
  process.exit(0)
}

const pkgPath = p('package.json')
const pkg = readJson(pkgPath)
const oldVersion = pkg.version
const newVersion = type ? bump(oldVersion, type) : oldVersion

if (!SEMVER.test(newVersion)) {
  console.error(`版本号格式不是 x.y.z：${newVersion}`)
  process.exit(1)
}

const results = []

// 1. package.json
if (newVersion !== oldVersion) {
  pkg.version = newVersion
  writeJson(pkgPath, pkg)
  results.push(`package.json                    ${oldVersion} → ${newVersion}`)
} else {
  results.push(`package.json                    ${newVersion}（未递增，仅同步）`)
}

// 2. package-lock.json
const lockPath = p('package-lock.json')
if (existsSync(lockPath)) {
  const lock = readJson(lockPath)
  lock.version = newVersion
  if (lock.packages && lock.packages['']) lock.packages[''].version = newVersion
  writeJson(lockPath, lock)
  results.push(`package-lock.json               ${newVersion}`)
}

// 3. src-tauri/tauri.conf.json（第一个 "version" 即为顶层版本）
if (
  replaceOnce(p('src-tauri', 'tauri.conf.json'), /("version"\s*:\s*")[^"]*(")/, `$1${newVersion}$2`)
) {
  results.push(`src-tauri/tauri.conf.json       ${newVersion}`)
}

// 4. src-tauri/Cargo.toml（[package] 段内的 version）
if (replaceOnce(p('src-tauri', 'Cargo.toml'), /^(version\s*=\s*")[^"]*(")/m, `$1${newVersion}$2`)) {
  results.push(`src-tauri/Cargo.toml            ${newVersion}`)
}

// 5. src-tauri/Cargo.lock（只改 set-hosts 这个包）
if (
  replaceOnce(
    p('src-tauri', 'Cargo.lock'),
    /(name = "set-hosts"\nversion = ")[^"]*(")/,
    `$1${newVersion}$2`,
  )
) {
  results.push(`src-tauri/Cargo.lock            ${newVersion}`)
}

// 6. Android 版本号与版本码
const propsPath = p('src-tauri', 'gen', 'android', 'app', 'tauri.properties')
if (existsSync(propsPath)) {
  const src = readFileSync(propsPath, 'utf-8')
  const oldCode = Number(src.match(/versionCode\s*=\s*(\d+)/)?.[1] ?? 0)
  const oldName = src.match(/versionName\s*=\s*(\S*)/)?.[1] ?? ''
  const newCode = nextVersionCode(oldCode, oldName, newVersion)
  writeFileSync(
    propsPath,
    src
      .replace(/versionName\s*=\s*\S*/, `versionName=${newVersion}`)
      .replace(/versionCode\s*=\s*\d+/, `versionCode=${newCode}`),
    'utf-8',
  )
  results.push(`android/tauri.properties        ${newVersion} (versionCode ${newCode})`)
}

// 7. Android 内嵌的 tauri.conf.json 副本
const androidConf = p(
  'src-tauri',
  'gen',
  'android',
  'app',
  'src',
  'main',
  'assets',
  'tauri.conf.json',
)
if (replaceOnce(androidConf, /("version"\s*:\s*")[^"]*(")/, `$1${newVersion}$2`)) {
  results.push(`android/assets/tauri.conf.json  ${newVersion}`)
}

// 8. iOS 工程版本号（gen/apple 存在时才同步）
const iosProjects = findFiles(p('src-tauri', 'gen', 'apple'), 'project.pbxproj')
let iosUpdated = 0
for (const proj of iosProjects) {
  if (replaceOnce(proj, /MARKETING_VERSION = [^;]+;/g, `MARKETING_VERSION = ${newVersion};`)) {
    iosUpdated += 1
  }
}
if (iosUpdated) results.push(`apple/project.pbxproj            ${newVersion}（${iosUpdated} 处）`)

console.log('\n版本同步完成：')
for (const line of results) console.log('  ' + line)
console.log(`\n当前版本：${newVersion}`)

// ---- git：提交版本文件并打标签 ----
// 只在递增版本时执行（version:sync 不产生新版本，无需打标签）
if (type && !noGit) {
  const tag = `v${newVersion}`
  const relativeOf = (file) => file.slice(root.length + 1).replace(/\\/g, '/')
  const versionFiles = [
    'package.json',
    'package-lock.json',
    'src-tauri/tauri.conf.json',
    'src-tauri/Cargo.toml',
    'src-tauri/Cargo.lock',
    'src-tauri/gen/android/app/tauri.properties',
    'src-tauri/gen/android/app/src/main/assets/tauri.conf.json',
    ...iosProjects.map(relativeOf),
  ].filter((f) => existsSync(p(f)))

  if (git('rev-parse', '--is-inside-work-tree').status !== 0) {
    console.log('\n[git] 当前目录不是 git 仓库，跳过提交与打标签')
  } else {
    let committed = !noCommit

    if (!noCommit) {
      // 只暂存版本相关文件，避免把无关的本地改动一起提交
      git('add', '--', ...versionFiles)
      const staged = git('diff', '--cached', '--name-only').stdout.trim()
      if (!staged) {
        console.log('\n[git] 版本文件无变化，跳过提交')
      } else {
        const commit = git('commit', '-m', `chore: bump version to ${newVersion}`)
        if (commit.status === 0) {
          console.log(`\n[git] 已提交：chore: bump version to ${newVersion}`)
        } else {
          committed = false
          console.log(`\n[git] 提交失败，跳过打标签：${(commit.stderr || '').trim()}`)
        }
      }
    }

    if (committed) {
      const tagExists = git('rev-parse', '-q', '--verify', `refs/tags/${tag}`).status === 0
      if (tagExists && !forceTag) {
        console.log(`[git] 标签 ${tag} 已存在，未重复创建（如需覆盖：--force-tag）`)
      } else {
        if (tagExists) git('tag', '-d', tag)
        const created = git('tag', '-a', tag, '-m', `Release ${tag}`)
        if (created.status === 0) {
          console.log(`[git] 已创建标签：${tag}`)
          console.log(`[git] 推送代码与标签：npm run push（git push --follow-tags）`)
        } else {
          console.log(`[git] 创建标签失败：${(created.stderr || '').trim()}`)
        }
      }
    }
  }
} else if (type && noGit) {
  console.log('\n[git] 已按 --no-git 跳过提交与打标签，可手动执行：')
  console.log(`  git add -A && git commit -m "chore: bump version to ${newVersion}"`)
  console.log(`  git tag -a v${newVersion} -m "Release v${newVersion}"`)
}
console.log('')
