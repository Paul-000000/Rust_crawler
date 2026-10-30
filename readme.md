# Crawler

Un crawler web rapide développé en **Rust**. Ce projet exploite le modèle de concurrence et la sécurité mémoire de Rust pour explorer, parser et extraire des données du Web de manière rapide et fiable.

## Fonctionnalités

- **Parcours Asynchrone / Multi-threadé** : Exploration simultanée de plusieurs pages web pour maximiser le débit.
- **Parsing HTML & Extraction** : Analyse efficace du DOM pour récupérer liens, métadonnées et contenus ciblés.
- **Respect du Roboting & Politesse** : Prise en charge des limites de débit (*rate limiting*) et du fichier `robots.txt`.
- **Gestion Stricte des Erreurs** : Résilience face aux échecs réseau, erreurs HTTP (4xx, 5xx) et timeouts.

## 📁 Structure du Projet

```text
.
├── Cargo.toml      # Fichier de configuration du projet et gestion des dépendances
├── Cargo.lock
└── src/
    └── main.rs      # Point d'entrée principal du code source
```

## Prérequis

* **Rust** et **Cargo** (installables via `rustup` ou le gestionnaire de paquets de votre système).

## Compilation et Exécution

2. **Lancer l'application :**
   ```bash
   cargo run
   ```

3. **Compiler :**
   ```bash
   cargo build --release
   ```