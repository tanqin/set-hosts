# Set Hosts

🌐 [English](README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Português (Brasil)](README.pt.md)

Uma ferramenta multiplataforma para gerenciar o arquivo hosts (interface inspirada no SwitchHosts), construída com **Tauri 2 + Vue 3 + TypeScript + Rust**: múltiplos perfis, assinatura de hosts remotos, dois modos de escrita (anexar / sobrescrever), backup e restauração automáticos e suporte a proxy para o download remoto. No desktop, o arquivo hosts do sistema é gravado com elevação automática de privilégios (UAC no Windows) e o cache DNS é limpo; no mobile, uma autorização de VPN do sistema assume o DNS para que os mapeamentos tenham efeito.

## Funcionalidades

### Gerenciamento de perfis

- **Perfis locais**: criar, renomear, excluir, editar o texto hosts e ativar / desativar / aplicar com um clique
- **Perfis remotos**: assinar uma URL `http(s)://` e baixar imediatamente ao criar; nome, URL e intervalo de atualização automática permanecem editáveis (ao alterar a URL, baixa novamente e, se ativo, reaplica)
- **Atualização automática**: intervalo próprio por perfil remoto (nunca / 1 min / 5 min / 15 min / 1 h / 24 h / 7 d), verificado a cada 30 segundos em segundo plano; também é possível ativar a atualização na inicialização
- **Mesclagem**: as entradas de todos os perfis ativos são gravadas juntas no «bloco gerenciado» do arquivo hosts do sistema

### Escrita do arquivo hosts do sistema (desktop)

- **Modo anexar (padrão)**: as entradas vão para o bloco gerenciado no fim do arquivo, **preservando as entradas existentes** (por exemplo `127.0.0.1 localhost`)
- **Modo sobrescrever**: o conteúdo dos perfis ativos **substitui integralmente** o arquivo hosts (inclua `localhost` em um perfil)
- O bloco gerenciado é delimitado por `# >>> Set Hosts Managed >>>` / `# <<< Set Hosts Managed <<<`; ao aplicar, apenas esse bloco é alterado
- No Windows, elevação automática de privilégios (UAC) e limpeza do cache DNS após gravar; macOS / Linux gravam em seus caminhos padrão

### Segurança e backups

- **Backup automático antes de cada gravação** do arquivo hosts do sistema, mantendo as últimas 50 cópias
- Backup manual, lista de backups e restauração com um clique

### Proxy DNS integrado (solução mobile)

O desktop altera diretamente o arquivo hosts do sistema; no mobile isso não é possível, portanto os mapeamentos passam a valer por meio de um servidor DNS local integrado mais uma VPN do sistema:

- **Totalmente automático**: ao ativar um perfil, o backend inicia o servidor DNS local e solicita a autorização de VPN do sistema — não há nenhuma opção relacionada na interface
- **Escuta em `127.0.0.1:5353`** por padrão (porta não privilegiada, sem root / administrador); se a porta estiver ocupada, usa-se automaticamente uma porta aleatória do sistema para garantir a inicialização
- **Mapeamentos são respondidos diretamente** com `A` / `AAAA`, TTL de 1 segundo (ligar/desligar tem efeito imediato); **o restante é encaminhado ao DNS upstream** (UDP, com nova consulta por TCP se a resposta for truncada)
- **Upstream detectado automaticamente**: o DNS do sistema é detectado automaticamente (`/etc/resolv.conf`, `ipconfig`); o proxy DNS não possui opções na interface — porta e upstream são decididos pelo backend
- **Atualização a quente**: os mapeamentos são ressincronizados após editar, ativar, atualizar remotos ou importar, sem reiniciar

### Proxy para download remoto

- O download de hosts remotos pode usar um proxy **HTTP / HTTPS / SOCKS5**, que afeta apenas as requisições desta aplicação

### Importação / Exportação

- **JSON**: exportação / importação da configuração completa (todos os perfis e backups)
- **Texto hosts**: exportar o texto hosts bruto do perfil atual ou importar texto hosts como um novo perfil
- Exportar para arquivo / importar de arquivo

### Configurações do aplicativo

- **Idioma da interface**: 简体中文、繁體中文、English、日本語、한국어、Deutsch、Français、Español、Português (Brasil) — 9 idiomas, com efeito imediato ao trocar
- Tema: claro / escuro
- Ocultar na bandeja ao iniciar, iniciar com o sistema (desktop)
- Modo de escrita, proxy para download remoto, atualização automática na inicialização
- Informações da plataforma: sistema operacional, desktop / mobile, caminho do arquivo hosts (abrir a pasta com um clique)
- Diretório de dados personalizado (migrável)

## Stack tecnológica

| Camada | Tecnologia |
|---|---|
| Frontend | Vue 3 (`<script setup>`) + TypeScript + Pinia + Element Plus + Vite |
| Casca | Tauri 2 (plugins tray-icon, dialog, shell, store, notification, autostart, single-instance) |
| Backend | Rust: tokio, hickory-proto (DNS), reqwest (download remoto; TLS do sistema no desktop, rustls no mobile), serde, chrono, uuid |
| Nativo mobile | Android `VpnService` (`DnsVpnService.kt`, ligado ao Rust via JNI) |

## Desenvolvimento

```bash
npm install                  # instalar dependências
npm run tauri dev            # modo de desenvolvimento no desktop
npm run dev                  # apenas frontend (sem ambiente Tauri, alguns recursos indisponíveis)
cd src-tauri && cargo test   # testes unitários em Rust
```

Desenvolvimento mobile:

```bash
npm run tauri android dev    # Android (requer JDK 17+ e Android SDK / NDK)
npm run tauri ios dev        # iOS (requer macOS + Xcode)
```

## Build e publicação

Compilar de uma vez todos os destinos que a máquina atual suporta (desktop + Android, mais iOS no macOS; os destinos são independentes):

```bash
npm run build:all
```

| Comando | Descrição |
|---|---|
| `npm run build:check` | Verificar o ambiente de build (JDK / Android SDK / NDK / target do rustup), sem compilar |
| `npm run build:desktop` | Instalador de desktop para o sistema atual (Windows `.exe` / `.msi`, macOS `.dmg`, Linux `.deb` / `.rpm` / `.AppImage`) |
| `npm run build:windows` / `build:macos` / `build:linux` | Sistema de destino explícito; erro imediato se não corresponder |
| `npm run build:android` | APK Android (arm64 por padrão) |
| `npm run build:android:all` | todas as ABIs em um pacote universal |
| `npm run build:android:split` | um pacote por ABI (menor) |
| `npm run build:android:debug` | pacote de depuração (sem assinatura, depurável) |
| `npm run build:android:aab` | AAB para a Google Play |
| `npm run build:ios` | iOS (requer macOS + Xcode) |

Os artefatos ficam na raiz do projeto: `dist-desktop/`, `dist-apk/`, `dist-ios/`.

Para um controle mais fino, execute os scripts diretamente (no Windows PowerShell, o npm engole flags com valor em `npm run xxx -- --flag value`):

```bash
node scripts/build-android.mjs --targets aarch64,armv7 --split-per-abi
node scripts/build-all.mjs --only android
node scripts/build-android.mjs --help
```

Ambiente de build Android: JDK 17+ e Android SDK (com NDK). Os scripts detectam os locais de instalação comuns; também é possível informar `--java-home` / `--sdk` ou definir `JAVA_HOME` / `ANDROID_HOME`. Se faltar um target do rustup, siga as dicas de `npm run build:check` (por exemplo `rustup target add aarch64-linux-android`).

### Gerenciamento de versão

O `package.json` é a única fonte da verdade; `scripts/bump-version.mjs` sincroniza **8 locais**: `package.json`, `package-lock.json`, `tauri.conf.json`, `Cargo.toml`, `Cargo.lock`, Android `tauri.properties` (`versionName` / `versionCode`), o `tauri.conf.json` embutido do Android e o `project.pbxproj` do iOS.

| Comando | Descrição |
|---|---|
| `npm run version:patch` / `version:minor` / `version:major` | Incrementar versão → sincronizar todos os arquivos → commit → criar a tag anotada `v*` |
| `npm run version:sync` | sem incrementar, apenas alinhar os demais arquivos à versão atual do `package.json` |
| `npm run version:check` | verificar se todas as versões coincidem (código de saída diferente de 0 se não) |
| `npm run push` | `git push --follow-tags`: enviar commits e tags juntos |

A versão exibida na página «Sobre» é injetada no build a partir do `package.json`, portanto cada destino traz sempre o número de versão mais recente.

## Como funciona

### Desktop: escrita do arquivo hosts do sistema

1. Cada perfil armazena o texto hosts bruto (editado localmente ou baixado)
2. Ao ativar / desativar / aplicar, o backend coleta as entradas de **todos os perfis ativos**
3. O conteúdo é gerado conforme o modo de escrita: anexar = remover o bloco gerenciado antigo e anexar o novo; sobrescrever = manter apenas o novo bloco gerenciado
4. Antes da gravação com elevação de privilégios, o hosts atual é copiado e, depois, o cache DNS é limpo

### Mobile: proxy DNS integrado + VPN

1. Todos os perfis ativos são compilados em uma tabela `domínio → IP` (em caso de duplicata, prevalece o perfil que aparece primeiro)
2. A tabela é recompilada e trocada a quente a cada alteração de configuração, sem reiniciar
3. Ao receber uma consulta: há mapeamento → responde diretamente `A` / `AAAA` (resposta vazia se a família de endereços não corresponder, evitando cair no DNS real); sem mapeamento ou tipos `CNAME` / `MX` etc. → encaminha ao upstream e devolve como está
4. No Android, o `DnsVpnService` aponta o DNS do sistema para o servidor local, de modo que os mapeamentos valem para todo o sistema

## Suporte de plataformas

| Plataforma | Estado |
|---|---|
| Windows / macOS / Linux | ✅ Suporte completo: escrita do arquivo hosts do sistema, backups automáticos, assinaturas remotas, importação / exportação |
| Android | ✅ Suportado: proxy DNS integrado + `VpnService` assume o DNS do sistema (na primeira ativação de um perfil aparece uma autorização de VPN do sistema, lembrada por pacote) |
| iOS | 🚧 Parcial: o proxy DNS e o frontend estão prontos, o túnel `Network Extension` ainda não foi conectado (requer conta de desenvolvedor paga e entitlement); até lá os mapeamentos não valem para todo o sistema |

## Estrutura do projeto

```
src/            Frontend (Vue 3 + TS): páginas, componentes, stores Pinia, i18n (9 idiomas)
src-tauri/      Backend Rust: comandos, análise de hosts, backups, proxy DNS, ponte nativa mobile
scripts/        Scripts de build e de gerenciamento de versão
dist-desktop/   Artefatos dos instaladores de desktop
dist-apk/       Artefatos Android
```

## Observações

- A primeira gravação no Windows exibe a solicitação de elevação UAC — é normal
- O modo sobrescrever remove as entradas hosts existentes (incluindo `localhost`); mantenha-as em um perfil se precisar delas. Em caso de erro, restaure em «Backup e restauração»
- Os hosts remotos são limitados a 8 MB com tempo limite de 15 segundos; o conteúdo deve ser texto hosts padrão (`IP domínio` por linha); não há suporte a JSON
- No Android, a primeira ativação de um perfil exige conceder a autorização de VPN; se recusada, o interruptor é revertido e a solicitação será feita novamente na próxima vez
- O tempo limite de uma consulta ao upstream do proxy DNS é de 4 segundos; em caso de falha, retorna `SERVFAIL`

## Licença

[Apache License 2.0](LICENSE) © 2026 tanqin
