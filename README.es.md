# Set Hosts

🌐 [English](README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Português (Brasil)](README.pt.md)

Una herramienta multiplataforma para gestionar el archivo hosts (interfaz inspirada en SwitchHosts), construida con **Tauri 2 + Vue 3 + TypeScript + Rust**: gestión de múltiples perfiles, suscripciones a hosts remotos, dos modos de escritura (añadir / sobrescribir), copias de seguridad y restauración automáticas, y soporte de proxy para la descarga remota. En el escritorio se escribe el archivo hosts del sistema con elevación de privilegios automática (UAC en Windows) y se vacía la caché DNS; en móviles, una autorización VPN del sistema asume el DNS para que los mapeos surtan efecto.

## Funciones

### Gestión de perfiles

- **Perfiles locales**: crear, renombrar, eliminar, editar el texto hosts y activar / desactivar / aplicar con un clic
- **Perfiles remotos**: suscribirse a una URL `http(s)://` y descargar inmediatamente al crearlos; el nombre, la URL y el intervalo de actualización automática siguen siendo editables (al cambiar la URL se vuelve a descargar y, si está activo, se vuelve a aplicar)
- **Actualización automática**: intervalo propio por perfil remoto (nunca / 1 min / 5 min / 15 min / 1 h / 24 h / 7 d), comprobado cada 30 segundos en segundo plano; también se puede activar la actualización al iniciar
- **Combinación**: las entradas de todos los perfiles activos se escriben juntas en el «bloque gestionado» del archivo hosts del sistema

### Escritura del archivo hosts del sistema (escritorio)

- **Modo añadir (predeterminado)**: las entradas se escriben en el bloque gestionado al final del archivo, **conservando las entradas existentes** (p. ej. `127.0.0.1 localhost`)
- **Modo sobrescribir**: el contenido de los perfiles activos **sustituye por completo** el archivo hosts (incluya `localhost` en un perfil)
- El bloque gestionado está delimitado por `# >>> Set Hosts Managed >>>` / `# <<< Set Hosts Managed <<<`; al aplicar solo se modifica ese bloque
- En Windows, elevación de privilegios automática (UAC) y vaciado de la caché DNS después de escribir; macOS / Linux escriben en sus rutas estándar

### Seguridad y copias de seguridad

- **Copia de seguridad automática antes de cada escritura** del archivo hosts del sistema, conservando las últimas 50 copias
- Copia manual, lista de copias y restauración con un clic

### Proxy DNS integrado (solución móvil)

El escritorio modifica directamente el archivo hosts del sistema; en móviles no es posible, por lo que los mapeos se aplican mediante un servidor DNS local integrado más un VPN del sistema:

- **Totalmente automático**: al activar un perfil, el backend inicia el servidor DNS local y solicita la autorización VPN del sistema — no hay ninguna opción relacionada en la interfaz
- **Escucha en `127.0.0.1:5353`** por defecto (puerto no privilegiado, sin root / administrador); si el puerto está ocupado, se recurre automáticamente a un puerto aleatorio del sistema para garantizar el arranque
- **Los mapeos se responden directamente** con `A` / `AAAA`, TTL de 1 segundo (el cambio surte efecto al instante); **el resto se reenvía al DNS ascendente** (UDP, con reintento por TCP si la respuesta está truncada)
- **Upstream detectado automáticamente**: se detecta el DNS del sistema (`/etc/resolv.conf`, `ipconfig`); el proxy DNS no tiene opciones en la interfaz: el puerto y el upstream los decide el backend
- **Actualización en caliente**: los mapeos se resincronizan tras editar, activar, actualizar remotos o importar, sin reiniciar

### Proxy para la descarga remota

- La descarga de hosts remotos puede usar un proxy **HTTP / HTTPS / SOCKS5**, que solo afecta a las peticiones de esta aplicación

### Importación / Exportación

- **JSON**: exportación / importación de la configuración completa (todos los perfiles y copias)
- **Texto hosts**: exportar el texto hosts sin procesar del perfil actual, o importar texto hosts como nuevo perfil
- Exportar a archivo / importar desde archivo

### Ajustes de la aplicación

- **Idioma de la interfaz**: 简体中文、繁體中文、English、日本語、한국어、Deutsch、Français、Español、Português (Brasil) — 9 idiomas, cambio inmediato
- Tema: claro / oscuro
- Ocultar en la bandeja al iniciar, iniciar con el sistema (escritorio)
- Modo de escritura, proxy para la descarga remota, actualización automática al iniciar
- Información de la plataforma: sistema operativo, escritorio / móvil, ruta del archivo hosts (abrir la carpeta con un clic)
- Directorio de datos personalizado (migrable)

## Stack tecnológico

| Capa | Tecnología |
|---|---|
| Frontend | Vue 3 (`<script setup>`) + TypeScript + Pinia + Element Plus + Vite |
| Envoltorio | Tauri 2 (plugins tray-icon, dialog, shell, store, notification, autostart, single-instance) |
| Backend | Rust: tokio, hickory-proto (DNS), reqwest (descarga remota; TLS del sistema en escritorio, rustls en móvil), serde, chrono, uuid |
| Nativo móvil | Android `VpnService` (`DnsVpnService.kt`, conectado a Rust mediante JNI) |

## Desarrollo

```bash
npm install                  # instalar dependencias
npm run tauri dev            # modo desarrollo de escritorio
npm run dev                  # solo frontend (sin entorno Tauri, algunas funciones no disponibles)
cd src-tauri && cargo test   # pruebas unitarias de Rust
```

Desarrollo móvil:

```bash
npm run tauri android dev    # Android (requiere JDK 17+ y Android SDK / NDK)
npm run tauri ios dev        # iOS (requiere macOS + Xcode)
```

## Compilación y publicación

Compilar de una vez todos los destinos que admite la máquina actual (escritorio + Android, más iOS en macOS; los destinos son independientes):

```bash
npm run build:all
```

| Comando | Descripción |
|---|---|
| `npm run build:check` | Revisar el entorno de compilación (JDK / Android SDK / NDK / destino rustup), sin compilar |
| `npm run build:desktop` | Instalador de escritorio para el sistema actual (Windows `.exe` / `.msi`, macOS `.dmg`, Linux `.deb` / `.rpm` / `.AppImage`) |
| `npm run build:windows` / `build:macos` / `build:linux` | Sistema de destino explícito; error inmediato si no coincide |
| `npm run build:android` | APK de Android (arm64 por defecto) |
| `npm run build:android:all` | todas las ABI en un paquete universal |
| `npm run build:android:split` | un paquete por ABI (más pequeño) |
| `npm run build:android:debug` | paquete de depuración (sin firmar, depurable) |
| `npm run build:android:aab` | AAB para Google Play |
| `npm run build:ios` | iOS (requiere macOS + Xcode) |

Los artefactos se recogen en la raíz del proyecto: `dist-desktop/`, `dist-apk/`, `dist-ios/`.

Para un control más fino, ejecute los scripts directamente (en Windows PowerShell, npm se come los flags con valor en `npm run xxx -- --flag value`):

```bash
node scripts/build-android.mjs --targets aarch64,armv7 --split-per-abi
node scripts/build-all.mjs --only android
node scripts/build-android.mjs --help
```

Entorno de compilación Android: JDK 17+ y Android SDK (con NDK). Los scripts detectan las rutas habituales; también se puede indicar `--java-home` / `--sdk` o definir `JAVA_HOME` / `ANDROID_HOME`. Si falta un destino de rustup, siga las indicaciones de `npm run build:check` (p. ej. `rustup target add aarch64-linux-android`).

### Gestión de versiones

`package.json` es la única fuente de verdad; `scripts/bump-version.mjs` sincroniza **8 lugares**: `package.json`, `package-lock.json`, `tauri.conf.json`, `Cargo.toml`, `Cargo.lock`, Android `tauri.properties` (`versionName` / `versionCode`), el `tauri.conf.json` incrustado de Android y iOS `project.pbxproj`.

| Comando | Descripción |
|---|---|
| `npm run version:patch` / `version:minor` / `version:major` | Incrementar versión → sincronizar todos los archivos → commit → crear la etiqueta anotada `v*` |
| `npm run version:sync` | sin incrementar, solo alinear los demás archivos con la versión actual de `package.json` |
| `npm run version:check` | comprobar que todas las versiones coinciden (código de salida distinto de 0 si no) |
| `npm run push` | `git push --follow-tags`: empujar commits y etiquetas juntos |

La versión mostrada en la página «Acerca de» se inyecta en tiempo de compilación desde `package.json`, de modo que cada destino incluye siempre el número de versión más reciente.

## Funcionamiento

### Escritorio: escritura del archivo hosts del sistema

1. Cada perfil guarda el texto hosts sin procesar (editado localmente o descargado)
2. Al activar / desactivar / aplicar, el backend recoge las entradas de **todos los perfiles activos**
3. El contenido se genera según el modo de escritura: añadir = quitar el bloque gestionado anterior y añadir el nuevo; sobrescribir = conservar solo el nuevo bloque gestionado
4. Antes de escribir con elevación de privilegios se hace una copia del hosts actual y, después, se vacía la caché DNS

### Móvil: proxy DNS integrado + VPN

1. Todos los perfiles activos se compilan en una tabla `dominio → IP` (en caso de duplicado, prevalece el perfil que aparece primero)
2. La tabla se recompila y se intercambia en caliente ante cualquier cambio de configuración, sin reiniciar
3. Al recibir una consulta: coincide → responder directamente `A` / `AAAA` (respuesta vacía si la familia de direcciones no coincide, para no caer al DNS real); no coincide o tipos `CNAME` / `MX`, etc. → reenviar al servidor ascendente y devolver tal cual
4. En Android, `DnsVpnService` apunta el DNS del sistema al servidor local, de modo que los mapeos se aplican a todo el sistema

## Soporte de plataformas

| Plataforma | Estado |
|---|---|
| Windows / macOS / Linux | ✅ Compatible por completo: escritura del archivo hosts del sistema, copias automáticas, suscripciones remotas, importación / exportación |
| Android | ✅ Compatible: proxy DNS integrado + `VpnService` asume el DNS del sistema (la primera vez que se activa un perfil aparece una autorización VPN del sistema, que se recuerda por paquete) |
| iOS | 🚧 Parcial: el proxy DNS y el frontend están terminados, el túnel `Network Extension` aún no está conectado (requiere cuenta de desarrollador de pago y entitlement); hasta entonces los mapeos no se aplican a todo el sistema |

## Estructura del proyecto

```
src/            Frontend (Vue 3 + TS): páginas, componentes, stores de Pinia, i18n (9 idiomas)
src-tauri/      Backend Rust: comandos, análisis de hosts, copias, proxy DNS, puente nativo móvil
scripts/        Scripts de compilación y de gestión de versiones
dist-desktop/   Artefactos de los instaladores de escritorio
dist-apk/       Artefactos de Android
```

## Notas

- La primera escritura en Windows muestra la solicitud de elevación UAC — es normal
- El modo sobrescribir elimina las entradas hosts existentes (incluido `localhost`); consérvelas en un perfil si las necesita. Si se equivoca, restaure desde «Copias de seguridad y restauración»
- Los hosts remotos están limitados a 8 MB con un tiempo de espera de 15 segundos; el contenido debe ser texto hosts estándar (`IP dominio` por línea); no se admite JSON
- En Android, la primera activación de un perfil requiere conceder la autorización VPN; si se rechaza, el interruptor se revierte y se volverá a solicitar la próxima vez
- El tiempo de espera de una consulta al servidor ascendente del proxy DNS es de 4 segundos; en caso de fallo se devuelve `SERVFAIL`

## Licencia

[Apache License 2.0](LICENSE) © 2026 tanqin
