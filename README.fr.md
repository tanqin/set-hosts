# Set Hosts

🌐 [English](README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Português (Brasil)](README.pt.md)

Un outil multiplateforme de gestion du fichier hosts (interface inspirée de SwitchHosts), construit avec **Tauri 2 + Vue 3 + TypeScript + Rust** : gestion de plusieurs profils, abonnements à des hosts distants, deux modes d'écriture (ajout / remplacement), sauvegardes et restauration automatiques, et prise en charge d'un proxy pour le téléchargement distant. Sur ordinateur, le fichier hosts système est écrit avec élévation de privilèges automatique (UAC sous Windows) puis le cache DNS est vidé ; sur mobile, une autorisation VPN système prend en charge le DNS pour que les correspondances s'appliquent.

## Fonctionnalités

### Gestion des profils

- **Profils locaux** : créer, renommer, supprimer, éditer le texte hosts, et activer / désactiver / appliquer en un clic
- **Profils distants** : s'abonner à une URL `http(s)://` et récupérer immédiatement lors de l'ajout ; le nom, l'URL et l'intervalle d'actualisation automatique restent modifiables (changer l'URL relance immédiatement le téléchargement et réapplique si le profil est actif)
- **Actualisation automatique** : intervalle propre à chaque profil distant (jamais / 1 min / 5 min / 15 min / 1 h / 24 h / 7 j), vérifié toutes les 30 secondes en arrière-plan ; l'actualisation au démarrage peut aussi être activée
- **Fusion** : les entrées de tous les profils activés sont écrites ensemble dans le « bloc géré » du fichier hosts système

### Écriture du fichier hosts système (ordinateur)

- **Mode ajout (par défaut)** : les entrées sont ajoutées dans le bloc géré en fin de fichier, **les entrées existantes sont conservées** (par ex. `127.0.0.1 localhost`)
- **Mode remplacement** : le contenu des profils actifs **remplace entièrement** le fichier hosts (pensez à inclure `localhost` dans un profil)
- Le bloc géré est délimité par `# >>> Set Hosts Managed >>>` / `# <<< Set Hosts Managed <<<` ; l'application ne modifie que ce bloc
- Sous Windows, élévation de privilèges automatique (UAC) puis vidage du cache DNS ; macOS / Linux écrivent dans leurs chemins standards

### Sécurité et sauvegardes

- **Sauvegarde automatique avant chaque écriture** du fichier hosts système, en conservant les 50 dernières copies
- Sauvegarde manuelle, liste des sauvegardes et restauration en un clic

### Proxy DNS intégré (solution mobile)

L'ordinateur modifie directement le fichier hosts système ; ce n'est pas possible sur mobile, les correspondances s'appliquent donc via un serveur DNS local intégré plus un VPN système :

- **Entièrement automatique** : lors de l'activation d'un profil, le backend démarre le serveur DNS local et demande l'autorisation VPN système — aucune option correspondante dans l'interface
- **Écoute sur `127.0.0.1:5353`** par défaut (port non privilégié, aucun root / administrateur requis) ; si le port est occupé, repli automatique sur un port système aléatoire pour garantir le démarrage
- **Les correspondances sont répondues directement** en `A` / `AAAA`, TTL de 1 seconde (l'activation est immédiate) ; **le reste est transmis au DNS en amont** (UDP, avec nouvelle requête en TCP si la réponse est tronquée)
- **Upstream détecté automatiquement** : le DNS système est détecté automatiquement (`/etc/resolv.conf`, `ipconfig`) ; le proxy DNS n'a aucune option dans l'interface — le port et l'upstream sont décidés par le backend
- **Mise à jour à chaud** : les correspondances sont resynchronisées après modification, activation, actualisation distante ou import — aucun redémarrage nécessaire

### Proxy pour le téléchargement distant

- Le téléchargement des hosts distants peut passer par un proxy **HTTP / HTTPS / SOCKS5**, qui n'affecte que les requêtes de cette application

### Import / Export

- **JSON** : export / import de la configuration complète (tous les profils et sauvegardes)
- **Texte hosts** : export du texte hosts brut du profil courant, ou import d'un texte hosts comme nouveau profil
- Export vers un fichier / import depuis un fichier

### Paramètres de l'application

- **Langue de l'interface** : 简体中文、繁體中文、English、日本語、한국어、Deutsch、Français、Español、Português (Brasil) — 9 langues, changement immédiat
- Thème : clair / sombre
- Masquer dans la barre système au démarrage, lancement au démarrage de la session (ordinateur)
- Mode d'écriture, proxy pour le téléchargement distant, actualisation automatique au démarrage
- Informations sur la plateforme : système d'exploitation, ordinateur / mobile, chemin du fichier hosts (ouverture du dossier en un clic)
- Répertoire de données personnalisé (déplaçable)

## Stack technique

| Couche | Technologie |
|---|---|
| Frontend | Vue 3 (`<script setup>`) + TypeScript + Pinia + Element Plus + Vite |
| Coque | Tauri 2 (plugins tray-icon, dialog, shell, store, notification, autostart, single-instance) |
| Backend | Rust : tokio, hickory-proto (DNS), reqwest (téléchargement distant ; TLS système sur ordinateur, rustls sur mobile), serde, chrono, uuid |
| Natif mobile | Android `VpnService` (`DnsVpnService.kt`, relié à Rust via JNI) |

## Développement

```bash
npm install                  # installer les dépendances
npm run tauri dev            # mode développement ordinateur
npm run dev                  # frontend seul (sans environnement Tauri, certaines fonctions indisponibles)
cd src-tauri && cargo test   # tests unitaires Rust
```

Développement mobile :

```bash
npm run tauri android dev    # Android (JDK 17+ et Android SDK / NDK requis)
npm run tauri ios dev        # iOS (macOS + Xcode requis)
```

## Build et publication

Construire d'un coup toutes les cibles possibles sur la machine (ordinateur + Android, plus iOS sous macOS ; les cibles sont indépendantes) :

```bash
npm run build:all
```

| Commande | Description |
|---|---|
| `npm run build:check` | Vérifier l'environnement de build (JDK / Android SDK / NDK / cible rustup), sans construire |
| `npm run build:desktop` | Installateur pour le système courant (Windows `.exe` / `.msi`, macOS `.dmg`, Linux `.deb` / `.rpm` / `.AppImage`) |
| `npm run build:windows` / `build:macos` / `build:linux` | Système cible explicite ; erreur immédiate si incompatible |
| `npm run build:android` | APK Android (arm64 par défaut) |
| `npm run build:android:all` | toutes les ABI dans un paquet universel |
| `npm run build:android:split` | un paquet par ABI (plus léger) |
| `npm run build:android:debug` | paquet de débogage (non signé, débogable) |
| `npm run build:android:aab` | AAB pour Google Play |
| `npm run build:ios` | iOS (macOS + Xcode requis) |

Les artefacts sont regroupés à la racine du projet : `dist-desktop/`, `dist-apk/`, `dist-ios/`.

Pour un contrôle plus fin, lancez les scripts directement (sous Windows PowerShell, npm avale les flags avec valeurs dans `npm run xxx -- --flag value`) :

```bash
node scripts/build-android.mjs --targets aarch64,armv7 --split-per-abi
node scripts/build-all.mjs --only android
node scripts/build-android.mjs --help
```

Environnement de build Android : JDK 17+ et Android SDK (avec NDK). Les scripts détectent les emplacements habituels ; vous pouvez aussi passer `--java-home` / `--sdk` ou définir `JAVA_HOME` / `ANDROID_HOME`. S'il manque une cible rustup, suivez les indications de `npm run build:check` (par ex. `rustup target add aarch64-linux-android`).

### Gestion des versions

`package.json` est la source unique de vérité ; `scripts/bump-version.mjs` synchronise **8 emplacements** : `package.json`, `package-lock.json`, `tauri.conf.json`, `Cargo.toml`, `Cargo.lock`, Android `tauri.properties` (`versionName` / `versionCode`), le `tauri.conf.json` embarqué d'Android et iOS `project.pbxproj`.

| Commande | Description |
|---|---|
| `npm run version:patch` / `version:minor` / `version:major` | Incrémenter la version → synchroniser tous les fichiers → commit → créer le tag annoté `v*` |
| `npm run version:sync` | sans incrément, aligner les autres fichiers sur la version courante de `package.json` |
| `npm run version:check` | vérifier que toutes les versions concordent (code de sortie non nul sinon) |
| `npm run push` | `git push --follow-tags` : pousser commits et tags ensemble |

La version affichée sur la page « À propos » est injectée au build depuis `package.json` : chaque cible embarque toujours le numéro de version le plus récent.

## Fonctionnement

### Ordinateur : écriture du fichier hosts système

1. Chaque profil stocke le texte hosts brut (édité localement ou téléchargé)
2. Lors de l'activation / désactivation / application, le backend collecte les entrées de **tous les profils activés**
3. Le contenu est généré selon le mode d'écriture : ajout = retirer l'ancien bloc géré puis ajouter le nouveau ; remplacement = ne conserver que le nouveau bloc géré
4. Le fichier hosts actuel est sauvegardé avant l'écriture avec élévation de privilèges, puis le cache DNS est vidé

### Mobile : proxy DNS intégré + VPN

1. Tous les profils activés sont compilés en une table `domaine → IP` (en cas de doublon, le premier profil rencontré l'emporte)
2. La table est recompilée et échangée à chaud à chaque changement de configuration, sans redémarrage
3. À la réception d'une requête : correspondance → réponse directe `A` / `AAAA` (réponse vide si la famille d'adresses ne correspond pas, pour éviter le repli vers le DNS réel) ; absence de correspondance ou types `CNAME` / `MX` etc. → transfert en amont et renvoi à l'identique
4. Sous Android, `DnsVpnService` oriente le DNS système vers le serveur local : les correspondances s'appliquent à tout le système

## Support des plateformes

| Plateforme | État |
|---|---|
| Windows / macOS / Linux | ✅ Pris en charge complètement : écriture du fichier hosts système, sauvegardes automatiques, abonnements distants, import / export |
| Android | ✅ Pris en charge : proxy DNS intégré + `VpnService` prend en charge le DNS système (l'autorisation VPN système s'affiche une seule fois à la première activation et est mémorisée par paquet) |
| iOS | 🚧 Partiel : le proxy DNS et le frontend sont terminés, le tunnel `Network Extension` n'est pas encore branché (compte développeur payant et entitlement requis) ; jusque-là, les correspondances ne s'appliquent pas à tout le système |

## Structure du projet

```
src/            Frontend (Vue 3 + TS) : pages, composants, stores Pinia, i18n (9 langues)
src-tauri/      Backend Rust : commandes, parsing hosts, sauvegardes, proxy DNS, pont natif mobile
scripts/        Scripts de build et de gestion des versions
dist-desktop/   Artefacts des installateurs ordinateur
dist-apk/       Artefacts Android
```

## Remarques

- La première écriture sous Windows affiche une demande d'élévation UAC — c'est normal
- Le mode remplacement supprime les entrées hosts existantes (y compris `localhost`) : conservez-les dans un profil si besoin. En cas d'erreur, restaurez depuis « Sauvegarde et restauration »
- Les hosts distants sont limités à 8 Mo avec un délai de 15 secondes ; le contenu doit être du texte hosts standard (`IP domaine` par ligne), le JSON n'est pas pris en charge
- Sous Android, la première activation d'un profil requiert l'autorisation VPN ; en cas de refus, l'interrupteur est réinitialisé et la demande sera représentée à la prochaine activation
- Le délai d'une requête en amont du proxy DNS est de 4 secondes ; en cas d'échec, `SERVFAIL` est renvoyé
