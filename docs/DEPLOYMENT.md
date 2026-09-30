# Déploiement et infrastructure

Contexte : qui touche au `compose.yaml`, à l'image, aux migrations ou à la CI du projet. Le code et les fichiers du
dépôt sont la source de vérité (`AGENTS.md` D1).

## Cible d'installation

L'installation de référence tient en une image, un Postgres 17 et une GitHub App. L'image contient le binaire
`cihub-server`, qui exécute par défaut les quatre rôles (`api`, `scheduler`, `worker`, `notifier`) dans un
processus ; `CIHUB_ROLES` en choisit une partie pour donner à chaque rôle ses propres processus
(`docs/ARCHITECTURE.md`, « Passer à l'échelle »). L'assistant de première configuration, servi par le rôle `api`,
crée la GitHub App par le flux de manifeste. Le `docker compose` d'installation arrive au lot 9, le chart Helm au
lot 15.

## Développement

`compose.yaml`, à la racine, démarre seulement Postgres (`postgres:17.5-alpine`), publié sur `127.0.0.1:5432`
(`AGENTS.md` D12). Le superutilisateur est `postgres` (mot de passe `postgres`). Le service ponctuel
`postgres-setup` amène le rôle `cihub` (mot de passe `cihub`), qui n'est pas superutilisateur et peut créer des
bases pour les tests, et la base `cihub` qu'il possède, à l'état voulu (D38, D52). Il tourne à chaque démarrage :
un changement de sa commande s'applique sans effacer les données. Ces valeurs servent au développement seulement.

```sh
mise run db            # Postgres, puis postgres-setup
mise run db-reset      # efface les données, puis mise run db (après la réécriture d'une migration)
mise run migrate       # Postgres, puis cihub-server migrate
mise run dev           # migrate, puis cihub-server avec les quatre rôles et les logs pretty
docker compose down    # arrête Postgres et garde le volume
```

Les tâches `mise` sont listées dans `AGENTS.md` §5.

## Variables du serveur

`cihub-server` lit sa configuration dans des variables `CIHUB_*` (`AGENTS.md` D31). Une valeur invalide arrête le
processus au démarrage.

| Variable              | Défaut                              | Rôle                                                                        |
| --------------------- | ----------------------------------- | --------------------------------------------------------------------------- |
| `CIHUB_ROLES`         | `api,scheduler,worker,notifier`     | Rôles du processus, séparés par des virgules (`AGENTS.md` D8)               |
| `CIHUB_DATABASE_URL`  | aucun, obligatoire                  | `postgres://utilisateur:mot-de-passe@hôte/base`, secret ; l'utilisateur n'est ni superutilisateur ni `BYPASSRLS` |
| `CIHUB_INTERNAL_ADDR` | `127.0.0.1:8081`                    | Adresse de `/healthz` et `/readyz`, à ne pas exposer (`docs/SECURITY.md`)   |
| `CIHUB_LOG`           | `info`                              | Filtre des logs (`docs/OBSERVABILITY.md`)                                   |
| `CIHUB_LOG_FORMAT`    | `json`                              | `json` ou `pretty` (`docs/OBSERVABILITY.md`)                                |

Dans un conteneur, `CIHUB_INTERNAL_ADDR` doit écouter sur `0.0.0.0` pour que les sondes le joignent. `/healthz`
répond `200` tant que le processus tourne ; `/readyz` répond `503` quand Postgres ne répond pas en 2 s.

## Migrations

Les migrations sont explicites (`AGENTS.md` D14) :

- `cihub-server migrate` applique les migrations embarquées qui manquent, puis s'arrête.
- Au démarrage, `cihub-server` n'applique rien : il s'arrête si une migration embarquée manque en base. Une
  migration en base que le binaire ne connaît pas vient d'une version plus récente et n'empêche pas le démarrage.
- Une migration publiée reste rétrocompatible sur une version, pour que les anciens et les nouveaux processus
  coexistent pendant un déploiement.

| Environnement                   | Ce qui lance `cihub-server migrate`                    | Quand                                    |
| ------------------------------- | ------------------------------------------------------ | ---------------------------------------- |
| Développement                   | `mise run migrate`                                     | À la main                                |
| `docker compose` d'installation | service `cihub-migrate`, dont dépend `cihub-server`    | Avant `cihub-server`                     |
| Kubernetes                      | `Job` en hook Helm `pre-install` et `pre-upgrade`      | Avant le déploiement des nouveaux pods   |

## Outils

`mise install` installe toute la suite (`AGENTS.md` D17) : Rust (version fixée dans `mise.toml`, avec `clippy`,
`rustfmt` et `llvm-tools`, par `rustup`), `cargo-deny`, `cargo-nextest`, `cargo-llvm-cov`, `actionlint` et
`zizmor`. `mise.lock` fixe les versions et les sommes de contrôle pour Linux et macOS, en x64 et arm64 ; après un
changement de version, `mise lock` le met à jour.

## CI du projet

`.github/workflows/ci.yml` suit l'organisation de `home-operations/external-dns-unifi-webhook` (`AGENTS.md` D18) :

| Job             | Tâche                     |
| --------------- | ------------------------- |
| `Rust Lint`     | `mise run lint`           |
| `Rust Test`     | `mise run test`           |
| `Rust Coverage` | `mise run cover`          |
| `Workflow Lint` | `mise run lint-workflows` |
| `Build Success` | réunit les jobs ci-dessus |

`Build Success` est le statut requis de la branche `main` : il échoue si un job a échoué ou a été annulé. La CI
tourne sur les pushes sur `main` (sauf un changement de Markdown seul), les PR vers `main`, `merge_group` et à la
demande. Chaque job installe les outils avec `mise install --locked`. Chaque action est épinglée par le SHA de son
commit, avec la version en commentaire. `.github/actionlint.yaml` fait ignorer à `actionlint` le label
`ubuntu-26.04`, qu'il ne connaît pas encore. Un service Postgres s'ajoute au lot 2, pour
`mise run test-integration`.

## Versions

release-please (`.github/workflows/release-please.yaml`, `AGENTS.md` D19) tient une PR de version à partir des
titres de commits au format Conventional Commits. Quand cette PR est mergée, il crée le tag `vX.Y.Z` et la release,
et met à jour `CHANGELOG.md`, `[workspace.package].version` dans `Cargo.toml` et les versions des crates `cihub-*`
dans `Cargo.lock`. La CI ne tourne pas sur les PR de release-please, qui ne changent que ces fichiers. Le workflow
prend un jeton de la GitHub App de bot du projet (secrets `BOT_CLIENT_ID` et `BOT_APP_PRIVATE_KEY`). L'image et
sa publication arrivent au lot 9.

## Mises à jour des dépendances

Renovate (`.github/workflows/renovate.yaml`, `AGENTS.md` D21) tourne toutes les heures, à un push qui modifie
`.renovaterc.json5`, et à la demande (avec une option de test à blanc). Il ouvre des PR pour les actions GitHub,
les outils de `mise.toml` (avec `mise.lock`) et les crates de `Cargo.toml`, et tient un tableau de bord dans une
issue. Aucune PR n'est mergée automatiquement. Il utilise la même GitHub App de bot que release-please.
