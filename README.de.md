# Set Hosts

🌐 [简体中文](README.md) · [繁體中文](README.zh-TW.md) · [English](README.en.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Português (Brasil)](README.pt.md)

Ein plattformübergreifendes Tool zur Verwaltung der hosts-Datei (UI angelehnt an SwitchHosts), gebaut mit **Tauri 2 + Vue 3 + TypeScript + Rust**: mehrere Profile, Remote-Hosts-Abos, zwei Schreibmodi (Anhängen / Überschreiben), automatische Backups und Wiederherstellung sowie Proxy-Unterstützung beim Abruf. Auf dem Desktop wird die System-hosts-Datei mit automatischer Rechteerweiterung (Windows-UAC) geschrieben und der DNS-Cache geleert; auf Mobilgeräten übernimmt eine System-VPN-Berechtigung das DNS, damit die Zuordnungen wirken.

## Funktionen

### Profilverwaltung

- **Lokale Profile**: hosts-Text erstellen, umbenennen, löschen, bearbeiten sowie mit einem Klick aktivieren / deaktivieren / anwenden
- **Remote-Profile**: eine `http(s)://`-URL abonnieren und beim Hinzufügen sofort abrufen; Name, URL und Intervall der automatischen Aktualisierung bleiben jederzeit änderbar (bei geänderter URL wird sofort neu geladen und bei aktivem Profil erneut angewendet)
- **Automatische Aktualisierung**: pro Remote-Profil eigenes Intervall (nie / 1 Min. / 5 Min. / 15 Min. / 1 Std. / 24 Std. / 7 Tage), Abfrage im Hintergrund alle 30 Sekunden; Aktualisierung beim Start lässt sich ebenfalls aktivieren
- **Zusammenführen**: die Einträge aller aktiven Profile werden gemeinsam in den „verwalteten Block“ der System-hosts-Datei geschrieben

### Schreiben der System-hosts-Datei (Desktop)

- **Anhängen (Standard)**: Einträge werden an das Ende der hosts-Datei in den verwalteten Block geschrieben, **bestehende Einträge bleiben erhalten** (z. B. `127.0.0.1 localhost`)
- **Überschreiben**: der Inhalt der aktiven Profile **ersetzt** die hosts-Datei vollständig (`localhost` u. a. sollten in einem Profil stehen)
- Der verwaltete Block wird von `# >>> Set Hosts Managed >>>` / `# <<< Set Hosts Managed <<<` umschlossen; beim Anwenden wird nur dieser Block verändert
- Unter Windows automatische Rechteerweiterung (UAC) und anschließendes Leeren des DNS-Caches; macOS / Linux schreiben in ihre Standardpfade

### Sicherheit und Backups

- **Vor jedem Schreiben der System-hosts-Datei automatische Sicherung**, es werden die letzten 50 Kopien aufbewahrt
- Manuelles Backup, Liste der Backups und Wiederherstellung mit einem Klick

### Eingebauter DNS-Proxy (Mobilvariante)

Der Desktop schreibt die System-hosts-Datei direkt; mobil ist das nicht möglich, daher wirken Zuordnungen über einen eingebauten lokalen DNS-Server plus System-VPN:

- **Vollautomatisch**: beim Einschalten eines Profils startet das Backend den lokalen DNS-Server und fordert die System-VPN-Berechtigung an — in der Oberfläche gibt es dazu keine Einstellung
- **Standardmäßig `127.0.0.1:5353`** (kein privilegierter Port, kein root / Administrator nötig); ist der Port belegt, wird automatisch auf einen zufälligen Systemport ausgewichen, damit der Server sicher startet
- **Treffer werden direkt mit `A` / `AAAA` beantwortet**, TTL 1 Sekunde (Umschalten wirkt sofort); **alles andere wird an den Upstream-DNS weitergeleitet** (UDP, bei abgeschnittener Antwort automatisch erneut per TCP)
- **Upstream konfigurierbar**: standardmäßig automatische Erkennung des System-DNS (`/etc/resolv.conf`, `ipconfig`), alternativ manuell (z. B. `223.5.5.5, 8.8.8.8`)
- **Aktualisierung ohne Neustart**: Zuordnungen werden nach Bearbeiten, Umschalten, Remote-Aktualisierung oder Import sofort synchronisiert

### Proxy für den Remote-Abruf

- Für den Abruf von Remote-Hosts kann ein **HTTP- / HTTPS- / SOCKS5-Proxy** konfiguriert werden; er gilt nur für die Anfragen dieser Anwendung

### Import / Export

- **JSON**: vollständige Konfiguration (alle Profile und Backups) exportieren / importieren
- **Hosts-Text**: den rohen hosts-Text des aktuellen Profils exportieren oder hosts-Text als neues Profil importieren
- Export in Datei / Import aus Datei

### Anwendungseinstellungen

- **Oberflächensprache**: 简体中文、繁體中文、English、日本語、한국어、Deutsch、Français、Español、Português (Brasil) — 9 Sprachen, Umschalten wirkt sofort
- Design: hell / dunkel
- Beim Start ins Tray minimieren, Autostart (Desktop)
- Schreibmodus, Proxy für den Remote-Abruf, automatische Aktualisierung beim Start
- Plattforminformationen: Betriebssystem, Desktop / Mobil, hosts-Pfad (Ordner mit einem Klick öffnen)
- Eigenes Datenverzeichnis (verschiebbar)

## Technik

| Schicht | Technik |
|---|---|
| Frontend | Vue 3 (`<script setup>`) + TypeScript + Pinia + Element Plus + Vite |
| Hülle | Tauri 2 (Plugins tray-icon, dialog, shell, store, notification, autostart, single-instance) |
| Backend | Rust: tokio, hickory-proto (DNS), reqwest (Remote-Abruf; System-TLS auf dem Desktop, rustls mobil), serde, chrono, uuid |
| Mobil, nativ | Android `VpnService` (`DnsVpnService.kt`, über JNI mit Rust verbunden) |

## Entwicklung

```bash
npm install                  # Abhängigkeiten installieren
npm run tauri dev            # Desktop-Entwicklungsmodus
npm run dev                  # nur Frontend (ohne Tauri-Umgebung, einige Funktionen nicht verfügbar)
cd src-tauri && cargo test   # Rust-Unit-Tests
```

Mobile Entwicklung:

```bash
npm run tauri android dev    # Android (JDK 17+ sowie Android SDK / NDK nötig)
npm run tauri ios dev        # iOS (macOS + Xcode nötig)
```

## Build und Veröffentlichung

Alle auf dem aktuellen System möglichen Ziele auf einmal bauen (Desktop + Android, unter macOS zusätzlich iOS; die Ziele sind unabhängig):

```bash
npm run build:all
```

| Befehl | Beschreibung |
|---|---|
| `npm run build:check` | Build-Umgebung prüfen (JDK / Android SDK / NDK / rustup target), ohne zu bauen |
| `npm run build:desktop` | Desktop-Installer für das aktuelle System (Windows `.exe` / `.msi`, macOS `.dmg`, Linux `.deb` / `.rpm` / `.AppImage`) |
| `npm run build:windows` / `build:macos` / `build:linux` | Zielsystem explizit angeben; bei Abweichung sofortiger Fehler |
| `npm run build:android` | Android-APK (standardmäßig arm64) |
| `npm run build:android:all` | alle ABIs in einem Universalpaket |
| `npm run build:android:split` | ein Paket pro ABI (kleiner) |
| `npm run build:android:debug` | Debug-Paket (unsigniert, debugbar) |
| `npm run build:android:aab` | AAB für Google Play |
| `npm run build:ios` | iOS (macOS + Xcode nötig) |

Die Ergebnisse liegen im Projektstamm: `dist-desktop/`, `dist-apk/`, `dist-ios/`.

Für feinere Steuerung die Skripte direkt aufrufen (unter Windows PowerShell verschluckt npm bei `npm run xxx -- --flag value` Flags mit Werten):

```bash
node scripts/build-android.mjs --targets aarch64,armv7 --split-per-abi
node scripts/build-all.mjs --only android
node scripts/build-android.mjs --help
```

Android-Build-Umgebung: JDK 17+ und Android SDK (mit NDK). Die Skripte erkennen übliche Installationsorte automatisch; alternativ `--java-home` / `--sdk` oder `JAVA_HOME` / `ANDROID_HOME` setzen. Fehlt ein rustup target, den Hinweisen von `npm run build:check` folgen (z. B. `rustup target add aarch64-linux-android`).

### Versionsverwaltung

Die Version stammt ausschließlich aus `package.json`; `scripts/bump-version.mjs` synchronisiert **8 Stellen**: `package.json`, `package-lock.json`, `tauri.conf.json`, `Cargo.toml`, `Cargo.lock`, Android `tauri.properties` (`versionName` / `versionCode`), die eingebettete Android-Datei `tauri.conf.json` und iOS `project.pbxproj`.

| Befehl | Beschreibung |
|---|---|
| `npm run version:patch` / `version:minor` / `version:major` | Version erhöhen → alle Dateien synchronisieren → committen → annotierten Tag `v*` erstellen |
| `npm run version:sync` | nicht erhöhen, nur alle anderen Dateien auf die aktuelle `package.json`-Version bringen |
| `npm run version:check` | prüfen, ob alle Stellen dieselbe Version haben (Exit-Code ungleich 0 bei Abweichung) |
| `npm run push` | `git push --follow-tags`: Commits und Tags gemeinsam pushen |

Die auf der Info-Seite angezeigte Version wird beim Build aus `package.json` eingefügt, sodass jedes Ziel immer die aktuelle Versionsnummer enthält.

## Funktionsweise

### Desktop: Schreiben der System-hosts-Datei

1. Jedes Profil speichert den rohen hosts-Text (lokal bearbeitet oder remote geladen)
2. Beim Aktivieren / Deaktivieren / Anwenden sammelt das Backend die Einträge **aller aktiven Profile**
3. Je nach Schreibmodus wird der Inhalt erzeugt: Anhängen = alten verwalteten Block entfernen und neuen anhängen; Überschreiben = nur den neuen verwalteten Block behalten
4. Vor dem Schreiben mit Rechteerweiterung wird die aktuelle hosts-Datei gesichert, danach der DNS-Cache geleert

### Mobil: eingebauter DNS-Proxy + VPN

1. Alle aktiven Profile werden zu einer Tabelle `Domain → IP` kompiliert (bei doppelten Domains gilt das zuerst vorkommende Profil)
2. Bei Konfigurationsänderungen wird die Tabelle neu kompiliert und sofort eingetauscht — kein Neustart nötig
3. Bei einer Anfrage: Treffer → direkt `A` / `AAAA` antworten (bei nicht passender Adressfamilie leere Antwort, damit kein Rückfall auf das echte DNS erfolgt); kein Treffer oder `CNAME` / `MX` usw. → an den Upstream weiterleiten und unverändert zurückgeben
4. Unter Android richtet `DnsVpnService` das System-DNS auf den lokalen Server, sodass die Zuordnungen systemweit gelten

## Plattformunterstützung

| Plattform | Status |
|---|---|
| Windows / macOS / Linux | ✅ vollständig unterstützt: Schreiben der System-hosts-Datei, automatische Backups, Remote-Abos, Import / Export |
| Android | ✅ unterstützt: eingebauter DNS-Proxy + `VpnService` übernimmt das System-DNS (beim ersten Einschalten erscheint einmalig die System-VPN-Berechtigung, die pro Paket dauerhaft gespeichert wird) |
| iOS | 🚧 teilweise: DNS-Proxy und Frontend sind fertig, der `Network Extension`-Tunnel ist noch nicht angebunden (erfordert kostenpflichtiges Entwicklerkonto und Entitlement); bis dahin wirken Zuordnungen nicht systemweit |

## Verzeichnisstruktur

```
src/            Frontend (Vue 3 + TS): Seiten, Komponenten, Pinia-Stores, i18n (9 Sprachen)
src-tauri/      Rust-Backend: Kommandos, hosts-Parsing, Backups, DNS-Proxy, native Mobile-Brücke
scripts/        Build-Skripte und Skript zur Versionsverwaltung
dist-desktop/   Ergebnisse der Desktop-Installer
dist-apk/       Android-Ergebnisse
```

## Hinweise

- Beim ersten Schreiben unter Windows erscheint die UAC-Abfrage der Rechteerweiterung — das ist normal
- Der Überschreiben-Modus entfernt bestehende hosts-Einträge (einschließlich `localhost`); wer sie behalten will, nimmt sie in ein Profil auf. Versehen lassen sich über „Backup und Wiederherstellung“ korrigieren
- Remote-Hosts sind auf 8MB begrenzt, der Abruf hat ein Zeitlimit von 15 Sekunden; der Inhalt muss einfacher hosts-Text sein (`IP Domain` pro Zeile), JSON wird nicht unterstützt
- Unter Android muss beim ersten Einschalten eines Profils die VPN-Berechtigung erteilt werden; bei Ablehnung wird der Schalter zurückgesetzt und beim nächsten Einschalten erneut abgefragt
- Das Zeitlimit für Upstream-Anfragen des DNS-Proxys beträgt 4 Sekunden; bei Fehlern wird `SERVFAIL` zurückgegeben
