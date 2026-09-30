# CIHub : architecture et manuel de travail pour les contributeurs (humains et agents)

Contexte : **tout le monde, en premier.** À lire avant de toucher au dépôt : les contraintes auxquelles la
conception répond (§1), le cœur du système (§2), les principes sur lesquels une PR est jugée (§3), les décisions en
vigueur et les questions ouvertes (§4), et les règles de travail (§5). L'ordre de lecture commence par `GOAL.md` ;
§0 donne la carte de tous les autres documents.

> **Pas de compatibilité ascendante avant la première version publiée.** On ne doit rien aux formes précédentes
> du système : pas d'alias, de repli, de route dépréciée, d'ancienne clé de configuration. Quand une décision
> change une forme, **supprimer l'ancienne forme dans le même changement** : code, routes, clients, tests, docs.
> Si vous écrivez « encore accepté pour… » ou « legacy », arrêtez-vous et supprimez la chose. À partir de la
> première version publiée, deux choses deviennent durables : le journal d'événements (il doit rester rejouable) et
> les migrations publiées (en avant seulement, rétrocompatibles sur une version, voir `docs/DEPLOYMENT.md`).

CIHub est une file de merge pour GitHub, open source et auto-hébergée, écrite en Rust au-dessus de Postgres. Elle
prend les PR approuvées et vertes, les ordonne par branche cible, reteste chacune au-dessus de tout ce qui la
précède selon la stratégie de la file, et ne merge que ce qui passe encore. Son second module, `flakytests`,
conserve l'historique des résultats de tests pour dire si un test est instable. `GOAL.md` dit pourquoi, ce fichier
garde les règles.

## 0. Carte des documents (une seule place par fait : lier, ne pas copier)

**Le code est la source de vérité (D1).** Les documents disent l'intention, les règles et le vocabulaire commun ;
ils peuvent être en retard sur le code. Quand un document et le code divergent, le code fait foi et on corrige le
document.

| Document                 | Qui le lit, et quand                                                                                       |
| ------------------------ | ---------------------------------------------------------------------------------------------------------- |
| `GOAL.md`                | Tout le monde, en premier. La phrase, le tableau d'acceptation, ce qu'on n'optimise pas.                   |
| `AGENTS.md` (ce fichier) | Tout le monde. Contraintes §1, cœur §2, principes §3, décisions §4, règles §5.                             |
| `docs/LOTS.md`           | Qui commence un travail : les phases, les lots, leur contenu, leurs critères de sortie et leur suivi.      |
| `docs/ARCHITECTURE.md`   | Tout le monde, avant le premier changement de code : parcours d'un événement, rôles, workspace, crates.    |
| `docs/DOMAIN.md`         | Qui touche aux états, transitions et raisons : le contrat de vocabulaire.                                  |
| `docs/ALGORITHMS.md`     | Qui touche au noyau (`cihub-mq-core`) ou à la boucle de réconciliation (`cihub-mergequeue`).               |
| `docs/GITHUB.md`         | Qui touche à `cihub-github`, aux permissions, aux webhooks, aux surfaces sur la PR.                        |
| `docs/CI.md`             | Qui touche au contrat entre la file et la CI.                                                              |
| `docs/DATA_MODEL.md`     | Qui écrit une migration ou une requête, ou touche à la RLS.                                                |
| `docs/ANALYTICS.md`      | Qui touche à `cihub-analytics`, aux tables de faits, aux métriques produit.                                |
| `docs/FLAKY_TESTS.md`    | Qui touche au module `flakytests` (`cihub-flakytests`) ou au trait `FlakeOracle`.                          |
| `docs/API.md`            | Qui ajoute une route, un code d'erreur, un webhook sortant, une commande de CLI.                           |
| `docs/CONFIGURATION.md`  | Qui ajoute un réglage, une stratégie ou une clé du fichier `.cihub.yaml`.                                  |
| `docs/RELIABILITY.md`    | Qui touche à un job, un appel GitHub, la boucle de réparation. Chaque panne a un traitement nommé.         |
| `docs/SECURITY.md`       | Qui touche à l'authentification, aux jetons, aux secrets.                                                  |
| `docs/OBSERVABILITY.md`  | Qui écrit un log, ajoute une métrique ou une alerte. Les règles d'écriture des logs y sont.                |
| `docs/DEPLOYMENT.md`     | Qui touche au `compose.yaml`, à l'image, aux migrations, à la CI du projet.                                |
| `docs/COMPETITION.md`    | Qui justifie un écart avec Trunk, Mergify, GitHub, GitLab, bors, Zuul, Aviator, Graphite.                  |

---

## 1. Contraintes pour lesquelles on conçoit

### 1.1 L'installation

- **Une image, un Postgres 17, une GitHub App.** Pas de broker, pas de Redis, pas de stockage objet en v1. Chaque
  dépendance ajoutée est une étape de plus pour un auto-hébergeur ; elle doit être justifiée par une mesure.
- **Quatre rôles sans état** (`api`, `scheduler`, `worker`, `notifier`) dans un seul binaire, `cihub-server`.
  Par défaut, un processus exécute les quatre rôles ; `CIHUB_ROLES` en choisit une partie, pour donner à chaque
  rôle ses propres processus et le dimensionner seul (D8). Tout l'état est dans Postgres.
- **Moins de trente minutes** de `docker compose up` à la première PR mergée, avec l'assistant qui crée la GitHub
  App par le flux de manifeste.

### 1.2 GitHub

- **GitHub perd des webhooks et ne réessaie pas.** Il les livre aussi en double et dans le désordre. Rien ne doit
  dépendre de la livraison d'un webhook : la boucle de réparation relit ce qui compte.
- **Le budget est de 5 000 points par heure et par installation**, plus des limites secondaires. C'est une
  ressource partagée par toutes les files d'une installation.
- **GitHub est la vérité de l'état git**, Postgres celle de l'état de la file. En cas de désaccord, on croit
  GitHub et on enregistre la transition.
- **`mergeable` est calculé paresseusement par GitHub** (`UNKNOWN`) : on relit, on ne calcule pas.

### 1.3 La CI des utilisateurs

- On ne la lance jamais, on ne la modifie jamais. Le seul contrat est un SHA de test et des noms de statuts
  requis. Un nom qui ne rapporte jamais donne un timeout, pas un échec.
- La CI est lente (minutes à heures) et instable : un seul échec ne suffit jamais à désigner un coupable.

### 1.4 L'échelle visée

- Une instance standard : 1 000 organisations, 20 000 dépôts, 5 000 files, 50 000 soumissions par jour, un à
  deux milliers de webhooks par minute en pointe. Au plus 500 entrées vivantes par file.
- L'équipe : un développeur qui pilote des agents de code. D'où des lots d'une semaine, des interfaces figées et
  des critères de sortie automatisés (`docs/LOTS.md`).

---

## 2. Le cœur, en une page

Le détail est dans `docs/ARCHITECTURE.md` et `docs/ALGORITHMS.md` ; ce résumé sert à juger un changement.

- **Journal d'événements par file.** L'état d'une file est un journal en ajout seul, de faits (ce qui a été
  observé) et de décisions (ce que la file a choisi). Les tables `queue_items`, `test_groups`, `test_runs` sont des
  projections, reconstructibles par rejeu, avec un instantané tous les 500 événements.
- **Réconciliateur pur.** `plan(état, now) → plan` n'a aucune E/S. Le plan décrit un **état voulu** (les refs de
  test et leur composition, les PR à merger, les statuts et commentaires à afficher), jamais une suite d'appels.
- **Convergence.** On compare le plan à l'état observé et on n'émet que les jobs qui comblent l'écart. Chaque job
  vérifie l'état réel avant d'agir et n'a pas d'effet si l'état est déjà atteint. Son résultat revient dans l'inbox
  comme un événement.
- **Une file, une instance à la fois.** Un bail (ou `pg_try_advisory_xact_lock`) désigne le scheduler d'une file ;
  les transitions sont gardées en SQL (`UPDATE … WHERE id = $1 AND state = $2`).
- **Ingestion inconditionnelle.** Vérifier la signature, stocker, acquitter en moins de 2 s. Aucune décision.
- **Budget GitHub ordonnancé.** Chaque job a une classe : `interactive` > `merge` > `speculate` > `repair`. Quand
  le budget baisse, la spéculation est réduite en premier, le merge en dernier.
- **Modes dégradés explicites** par connexion : `normal`, `throttled`, `github_unavailable`, `paused`.

---

## 3. Principes (ce sur quoi une PR est jugée)

Une idée : **la file est un journal ; tout le reste est une projection du journal ou une convergence vers l'état
qu'il décrit.** Dix conséquences. Un changement qui semble naturel dans un service classique (appeler GitHub
depuis un handler, garder un état en mémoire, ajouter une file de messages) est souvent une violation ici. Une
violation veut dire que le principe est faux (le corriger par une décision en §4, validée selon §4) ou que la PR
l'est ; jamais « on corrigera plus tard ».

| #        | Principe                                                                                                                                                                                      | L'indice dans une PR                                                                                                                                                         | La question à se poser                                                                  |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| **I**    | **Le journal est la source de vérité de la file.** Les tables d'état sont des projections.                                                                                                    | Une écriture dans `queue_items` sans événement ; un état gardé en mémoire entre deux réconciliations.                                                                        | « Si je rejoue le journal de cette file, est-ce que j'obtiens le même état ? »          |
| **II**   | **Le noyau est pur.** Les crates pures (`cihub-domain`, `cihub-conditions`, `cihub-config`, `cihub-mq-core`, `cihub-flaky-core`) ne font aucune E/S et ne lisent ni l'horloge ni le hasard. | Une dépendance à `tokio`, `sqlx`, `reqwest` ou `axum` ; un appel à `OffsetDateTime::now_utc`, `SystemTime::now` ou `Ulid::generate` sans exception locale (D16) ; un trait de store ou de SCM dans le noyau. | « Ce test de table peut-il tourner sans Postgres ni GitHub ? »                          |
| **III**  | **Converger, ne pas piloter.** Un job décrit un état voulu et vérifie l'état réel avant d'agir.                                                                                               | Une séquence d'appels GitHub dans le réconciliateur ; un job qui échoue si l'état est déjà atteint.                                                                          | « Si ce job tourne deux fois, ou s'arrête au milieu et reprend, que se passe-t-il ? »   |
| **IV**   | **La branche cible reste verte.** Une PR ne merge qu'après un test sur un commit qui contient tous ses prédécesseurs.                                                                         | Un merge sans exécution valide ; des statuts requis lus depuis la branche de la PR.                                                                                          | « Quel commit a été testé, et est-ce exactement ce qui atterrit (ou son arbre) ? »      |
| **V**    | **Aucune intention perdue.** L'ingestion stocke avant de décider ; un webhook perdu est un retard.                                                                                            | Un traitement dans le handler de webhook ; une transition qui n'arrive que par un webhook.                                                                                   | « Si ce webhook n'arrive jamais, qui le rattrape, et en combien de temps ? »            |
| **VI**   | **Chaque décision s'explique.** Chaque transition porte une raison du vocabulaire fermé (`docs/DOMAIN.md`).                                                                                   | Une nouvelle raison hors du vocabulaire ; une sortie de file sans raison ; une chaîne libre en guise de raison.                                                              | « Que lit l'utilisateur sur sa PR, et que lit l'opérateur dans la chronologie ? »       |
| **VII**  | **Le budget GitHub est ordonnancé.** Tout appel passe par le trait `Scm`, porte une classe et consomme le budget de la connexion.                                                           | Un client `reqwest` vers GitHub hors de `cihub-github` ; un appel sans classe ; une boucle de lectures non groupées.                                                         | « Combien de points par heure coûte ce changement pour 50 PR en attente ? »             |
| **VIII** | **Isolation par organisation.** Chaque table de tenant porte `org_id` et la RLS ; chaque requête du scheduler filtre par `queue_id`.                                                          | Une table sans `org_id` ; une requête sans filtre ; un cache partagé entre organisations.                                                                                    | « Une organisation peut-elle lire, ralentir ou déduire quoi que ce soit d'une autre ? » |
| **IX**   | **Les modules ne se connaissent pas.** `cihub-mergequeue` et `cihub-flakytests` communiquent par les traits de `cihub-contracts` (`FlakeOracle`) et par les événements.                     | Une dépendance de `cihub-mergequeue` vers `cihub-flakytests`, ou l'inverse.                                                                                                  | « Si ce module est désactivé, la file se comporte-t-elle exactement comme avant ? »     |
| **X**    | **Garder CIHub petit.** Postgres seul ; une stratégie est une table de valeurs par défaut ; chaque dépendance est validée (D2).                                                                | Une nouvelle dépendance ou une nouvelle crate sans validation ; du code de stratégie dans le noyau ; une option sans utilisateur.                                             | « Quelle ligne de `GOAL.md` ce changement sert-il ? »                                   |

---

## 4. Décisions

**Une décision est validée avant d'être appliquée.** Un agent propose chaque choix d'architecture, de
dépendance, de crate, de vocabulaire, de format public ou de nom à l'utilisateur, ou à la personne qui développe
la feature, et attend son accord avant de l'écrire ou de l'appliquer. Un agent n'ajoute jamais une décision seul.

**Le journal des décisions est en ajout seul.** Chaque décision a un identifiant stable (`Dn`), une date et le nom
de la personne qui l'a validée. On ne modifie ni ne supprime une décision : pour la changer, on ajoute une nouvelle
décision qui dit ce qu'elle remplace, et on ajoute à l'ancienne la mention _Remplacée par Dn_ (superseded-by).

**29 septembre 2026, validées par Nicolas Lecoy.** Le projet passe de Go à Rust ; le journal repart de D1.

- **D1** Le code est la source de vérité. Les documents disent l'intention, les règles et le vocabulaire commun,
  et peuvent être en retard. `docs/DOMAIN.md` est le contrat de vocabulaire : il définit chaque terme et renvoie
  au code pour la liste des valeurs. Ce qui n'est pas encore codé est décrit dans le lot qui le construit
  (`docs/LOTS.md`), pas dans `docs/DOMAIN.md`.
- **D2** La liste des dépendances est `[workspace.dependencies]` dans `Cargo.toml`. Une nouvelle dépendance
  externe est validée selon §4.
- **D3** Chaque lot a un statut (`à faire`, `en cours`, `terminé`, `bloqué` avec la cause) et, dès qu'il commence,
  un bloc « Suivi » (fait, reste, écarts avec le plan) dans `docs/LOTS.md`, mis à jour dans le même changement que
  le code.
- **D4** Les identifiants sont des ULID typés au format texte `typedid` (`org_0627ank54ajth78mw4353dqpw5dvt5kp`),
  dans `cihub-domain`.
- **D5** Le journal d'une file contient des faits et des décisions ; le rejeu applique les événements avec la
  fonction pure `apply` et n'appelle jamais `plan` ; les identifiants des décisions sont des ULID déterministes.
  Le détail est la cible du lot 1 (`docs/LOTS.md`).
- **D6** Une seule énumération `Reason` et une table des transitions permises, dont la clé est (état de départ,
  état d'arrivée) et la valeur la liste des raisons permises, pour les entrées, les groupes, les exécutions et la
  file. Elle remplace les énumérations de raisons par état d'arrivée (lot 1).
- **D7** Pas de variante pour les PR empilées (`ConflictCulprit::StackedPr`, `stacked_pr_*`) avant le lot des PR
  empilées.
- **D8** Deux binaires. `cihub-server` (`crates/server/`) porte les quatre rôles, tout-en-un par défaut ;
  `CIHUB_ROLES` (`api`, `scheduler`, `worker`, `notifier`, séparés par des virgules) choisit ceux du processus. Pas
  de crate par rôle : un rôle lance des boucles dont le code est dans les crates de bibliothèque. Les rôles d'un
  même processus ne partagent que le pool Postgres. `cihub` est la CLI, distribuée seule, sans code serveur.
- **D9** Une crate se crée quand un lot en a besoin, après validation. `docs/ARCHITECTURE.md` décrit les crates qui
  existent et la liste cible. Le squelette de `cihub-server` arrive au lot 2.
- **D10** Les tâches sont dans `mise.toml` à la racine ; une tâche existe seulement quand elle a quelque chose à
  exécuter (§5). La CI appelle les tâches `mise`, sans répéter leurs commandes.
- **D11** Les crates pures ont une couverture de lignes d'au moins 90 %, mesurée par `cargo-llvm-cov`
  (`mise run cover`) et contrôlée en CI.
- **D12** `compose.yaml` à la racine démarre seulement Postgres (`postgres:17.5-alpine`) pour le développement,
  publié sur `127.0.0.1`, avec un mot de passe de développement écrit dans le fichier. _Complétée par D52._
- **D13** Les logs utilisent `tracing` et `tracing-subscriber`, en JSON par défaut ou en `pretty`, avec les clés de
  corrélation portées par les spans ; les secrets sont dans des `secrecy::SecretString`. Le format et les règles
  sont dans `docs/OBSERVABILITY.md`.
- **D14** Les migrations sont explicites. `cihub-server migrate` les applique puis s'arrête ; au démarrage,
  `cihub-server` n'applique rien et s'arrête si une migration embarquée manque en base. Un processus dédié les
  lance avant les rôles : `mise run migrate` en développement, un service `cihub-migrate` dans le `docker compose`
  d'installation, un `Job` en hook Helm `pre-install` et `pre-upgrade` sur Kubernetes.
- **D15** Les pratiques Rust de §5 (erreurs, lints, tests, asynchrone, dépendances, documentation du code).
- **D16** (Q11) L'horloge et le hasard sont interdits partout par `disallowed-methods` dans le `clippy.toml` de la
  racine (`SystemTime::now`, `Instant::now`, `OffsetDateTime::now_utc`, `Ulid::generate`, `Generator::generate`).
  Le seul endroit qui les lit porte un `#[expect(clippy::disallowed_methods, reason = "…")]` local.
- **D17** Toute la suite d'outils s'installe par `mise` : Rust (version fixée, profil `minimal`, composants
  `clippy`, `rustfmt` et `llvm-tools`) et les outils de `mise.toml`. `mise.lock` fixe les versions et les sommes de
  contrôle par plateforme ; la CI installe avec `--locked`.
- **D18** La CI suit l'organisation de `home-operations/external-dns-unifi-webhook` : un job par tâche `mise`, un
  job `Workflow Lint` (`actionlint`, `zizmor`), et un job `Build Success` qui les réunit et sert de statut requis.
  Chaque action est épinglée par le SHA de son commit, avec la version en commentaire
  (`actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1`).
- **D19** Les versions sont publiées par release-please (`release-please-config.json`,
  `.release-please-manifest.json`), à partir des titres de commits au format Conventional Commits. Type `simple` :
  le type `rust` ne gère pas un workspace dont la racine n'a pas de `[package]`. release-please met à jour
  `[workspace.package].version` dans `Cargo.toml`, les versions des crates `cihub-*` dans `Cargo.lock` et
  `CHANGELOG.md` ; les tags sont `vX.Y.Z`. Le workflow utilise la GitHub App de bot du projet (secrets
  `BOT_CLIENT_ID` et `BOT_APP_PRIVATE_KEY`). L'image et son workflow de publication arrivent au lot 9.
- **D20** Complète D16 : les constructeurs d'identifiants (`OrgId::new()` et les autres) sont aussi interdits par
  `disallowed-methods` ; ils portent l'exception locale et n'ont pas d'`impl Default`. Les tests utilisent des
  identifiants fixes (`OrgId::from_ulid(Ulid(1))`).
- **D21** Renovate tourne dans ce dépôt par `.github/workflows/renovate.yaml`, copié de `nlecoy/home-ops`, avec la
  GitHub App de bot, et peut exécuter `mise` pour mettre à jour `mise.lock`. Sa configuration `.renovaterc.json5`
  est propre au projet (pas de preset `home-operations`) : actions épinglées par SHA avec la version en commentaire,
  titres Conventional Commits, groupes (actions GitHub, outils `mise`, toolchain Rust), attente de 3 jours après une
  publication, mise à jour mensuelle des lockfiles. Pas de merge automatique : une personne merge chaque mise à
  jour.
- **D22** Complète D17 : un outil de `mise.toml` utilise un backend qui télécharge un binaire précompilé
  (`aqua:`, `github:`), avec sa somme de contrôle dans `mise.lock`. Le backend `cargo:`, qui compile l'outil
  depuis les sources dans chaque job de CI, sert seulement quand l'outil ne publie pas de binaire.
- **D23** Le journal utilise le vocabulaire de CIHub, pas celui d'un SCM. Chaque adaptateur traduit les valeurs
  de son SCM en faits neutres ; le noyau ne voit aucune valeur propre à GitHub. « PR » désigne toute demande de
  changement. `Mergeability` vaut `mergeable`, `behind`, `conflict`, `blocked` ou `unknown` ; les raisons d'échec
  du merge sont `merge_rejected`, `direct_merge_rejected`, `merge_rate_limit_exceeded`,
  `merge_branch_protection_unmet`, `merge_already_in_progress` et `merge_repeated_server_errors` ; la perte de
  l'accès au dépôt est un fait `access.lost` ou `access.restored` ; `CheckKind` vaut `check` ou `status`. Les noms
  de Trunk restent dans les webhooks sortants, par une traduction au lot 11.
- **D24** La crate pure `cihub-mq-core` (`crates/mq-core/`) contient `apply`, le graphe de merge, `plan` et
  `check`. Elle dépend de `cihub-domain` et ne connaît ni `cihub-config` ni `cihub-conditions`.
- **D25** Les réglages de la file (`QueueSettings`) sont un paramètre de `plan` et de `check`, comme l'heure :
  `plan(snapshot, settings, now)` et `check(snapshot, settings)`. Ils ne sont pas dans le snapshot, car
  `cihub-domain` ne peut pas dépendre de `cihub-config`. Le fait `queue.settings_changed` porte seulement le mode
  avant et après.
- **D26** `thiserror` pour les erreurs des crates de bibliothèque (§5.1) ; `proptest` pour les tests de
  propriété, en dépendance de développement seulement. `insta` arrive avec le premier lot qui compare des plans
  par instantanés.
- **D27** (Q6) La spéculation probabiliste reste hors v1. Le fait `item.submitted` garde dès maintenant les
  attributs dont elle aura besoin : `author`, `additions`, `deletions` et `changed_files`. Les durées viennent des
  horodatages du journal ; elles ne sont pas un attribut. _Complétée par D42 : `author` devient `author_id`._
- **D28** (Q10) Les tables de la file sont `queue_items`, `test_groups` et `test_runs`, sans préfixe de module.
- **D29** Le lot 2 crée les crates `cihub-server` et `cihub-store`.
- **D30** Dépendances du lot 2 : `tokio`, `tokio-util`, `sqlx` (Postgres, `rustls`), `axum`, `anyhow` (binaires
  seulement), `tracing`, `tracing-subscriber`, `secrecy`, `sha2` et `rand`. Toute autre dépendance est proposée
  selon §4.
- **D31** Toutes les variables d'environnement du serveur commencent par `CIHUB_`.
- **D32** Un jeton d'API est `cht_` suivi de 20 octets aléatoires encodés en base32 avec l'alphabet des typedid
  (36 caractères au total), envoyé par `Authorization: Bearer`. Le serveur garde seulement l'empreinte SHA-256 du
  jeton et le retrouve par cette empreinte. `cht` n'est jamais un type d'identifiant typedid.
- **D33** L'API est servie sous `/api/v1`, sur l'adresse de l'instance, à côté du tableau de bord.
- **D34** Complète D29 : le lot 2 crée aussi la crate `cihub-telemetry` (`crates/telemetry/`), qui installe les
  logs (D13), puis les traces OpenTelemetry et les métriques Prometheus quand elles arrivent. Seuls les binaires
  l'utilisent. En `pretty`, un log est une ligne : heure UTC en millisecondes, niveau, message et champs.
- **D35** Le contrat `Module` et l'activation d'un module par organisation arrivent au lot 5, avec le premier
  module (`cihub-mergequeue`), et non au lot 2.
- **D36** La table `events` est partitionnée par mois. `cihub-server migrate` crée les partitions du mois en cours
  et du mois suivant ; une boucle du rôle `scheduler` crée chaque jour la partition du mois suivant si elle
  manque, jusqu'à ce qu'un job la remplace au lot 3. Pas d'extension (`pg_partman`) ni de partition par défaut.
- **D37** Les colonnes d'identifiant sont en `text` et gardent la forme typedid (`org_0627…`, D4).
- **D38** Un seul rôle Postgres, celui de `CIHUB_DATABASE_URL`, possède les tables et exécute les requêtes. Chaque
  table qui porte `org_id` (et `orgs`) a la sécurité au niveau ligne, forcée aussi pour le propriétaire.
  `cihub-store` ouvre une transaction dans un de deux contextes : `begin_org(org_id)` (paramètre `cihub.org_id`)
  voit une organisation, `begin_system()` (paramètre `cihub.system`) les voit toutes ; une transaction sans
  contexte ne voit rien. Le rôle n'est ni superutilisateur ni `BYPASSRLS` : `cihub-server` refuse de démarrer
  sinon. Le `compose.yaml` de développement crée ce rôle (`cihub`) à côté du superutilisateur `postgres`.
- **D39** La table `users` est globale, sans `org_id` ni sécurité au niveau ligne : un utilisateur GitHub
  appartient à plusieurs organisations par `memberships`. _Complétée par D51._
- **D40** Nouvelles valeurs fermées : `MembershipRole` (`admin`, `member`, `viewer`), `AppKind` (`main`, `sudo`),
  `ApiScope` (`queues:read`, `queues:write`, `prs:write`, `targets:write`, `metrics:read`). Nouveaux types
  d'identifiant : `installation`, `user`, `apitoken` (la ligne du jeton ; le secret reste `cht_…`, D32), `audit`.
  _`AppKind` et le type `installation` remplacés par D49._
- **D41** Un fichier de migration s'appelle `<AAAAMMJJHHMMSS>_<description>.sql` (format de `sqlx migrate add`).
- **D42** Complète D27 : le journal ne garde aucun login. Un acteur utilisateur est `Actor::User { user_id }`,
  stocké en `events.actor_kind` (`system`, `bot`, `user`) et `events.actor_user_id`, qui pointe vers `users` ;
  `item.submitted` porte `author_id` et `target.pushed` porte `sender_id`. `users` garde le login : un
  renommage ou une anonymisation change une ligne, jamais le journal. L'ingestion crée ou met à jour la ligne
  `users` avant d'écrire l'événement (lots 4 et 5). _Complétée par D51._
- **D43** Table `events` : `id`, `org_id`, `queue_id`, `seq`, `type` (enum `event_type`), `actor_kind`,
  `actor_user_id`, `cause_id`, `item_id`, `group_id`, `run_id`, `payload` (les champs du type, sérialisés par
  `cihub-domain`) et `occurred_at` (à la microseconde) ; clé primaire (`id`, `occurred_at`). `seq` est le rang
  dans le journal de la file, à partir de 1 : `cihub-store` le calcule à l'écriture, et l'appelant tient le
  verrou de la file (lot 3). Une partition par mois UTC, `events_<AAAA>_<MM>`, avec sa propre politique forcée.
  La suppression des partitions de plus de 90 jours arrive au lot 3, avec les jobs.
- **D44** L'outbox, les abonnements et le critère « un événement et son entrée d'outbox sont écrits dans la même
  transaction » passent du lot 2 au lot 3, avec le trait `Outbox`. _Complétée par D62 : pas de trait. Remplacée par
  D65._
- **D45** Les jetons d'API sont dans `cihub-store` (création, authentification par empreinte dans le contexte
  système) ; `cihub-domain` porte leur forme texte, sans hasard. Le secret vient du générateur aléatoire du
  système (`SysRng`), interdit ailleurs par `clippy.toml`. `last_used_at` avance au plus une fois par minute,
  pour qu'un appel d'API n'écrive pas en base.

**30 septembre 2026, validées par Nicolas Lecoy.**

- **D46** Le modèle de données ne dépend d'aucun SCM, comme le journal (D23). Un identifiant du SCM est une colonne
  `external_id` en `text`, unique dans son instance de SCM seulement : chaque clé d'unicité porte
  `scm_instance_id`.
- **D47** Table `scm_instances` (`id`, `kind`, `base_url` unique) : github.com ou un GitHub Enterprise Server. Elle
  est globale, sans `org_id` ni sécurité au niveau ligne, car les organisations partagent une instance. Valeur
  fermée `ScmKind` (`github`) ; type d'identifiant `scminstance`. La migration ne crée aucune instance :
  l'assistant d'installation la crée (lot 4), car une GitHub App appartient à une seule instance.
- **D48** `orgs` n'a plus de colonne propre à un SCM ni de colonne `settings`. Table `scm_accounts` (`id`,
  `org_id`, `scm_instance_id`, `external_id`, `path`), unique (`scm_instance_id`, `external_id`) : le compte du SCM
  (une organisation GitHub) rattaché à une organisation, qui peut en avoir plusieurs. Type d'identifiant
  `scmaccount`.
- **D49** Table `scm_connections` (`id`, `org_id`, `scm_instance_id`, `scm_account_id`, `kind`, `external_id`,
  `suspended_at`, `claimed_at`), unique (`scm_instance_id`, `kind`, `external_id`) : le moyen par lequel CIHub
  atteint les dépôts d'un compte. Elle remplace `installations`. Valeur fermée `ConnectionKind` (`github_app`,
  `github_sudo_app`) à la place d'`AppKind` ; type d'identifiant `connection` à la place d'`installation`. Le
  budget, les quotas et les modes dégradés sont par connexion.
- **D50** `repos` : `scm_instance_id`, `connection_id`, `external_id`, `namespace`, `name`, `default_branch`,
  unique (`scm_instance_id`, `external_id`).
- **D51** Complète D39 et D42 : une ligne `users` est un compte d'utilisateur d'une instance de SCM
  (`scm_instance_id`, `external_id`, `login`), unique (`scm_instance_id`, `external_id`). Les comptes d'une même
  personne sont des lignes séparées, sans lien ; une table de personnes pourra les regrouper plus tard sans
  changer le journal, qui pointe vers le compte qui a agi.
- **D52** Complète D12 : `compose.yaml` ajoute un service ponctuel `postgres-setup`, qui amène le rôle `cihub` et
  la base `cihub` à l'état voulu à chaque démarrage (création s'ils manquent, puis `ALTER`). Il remplace le script
  d'initialisation, qui ne tournait que sur un volume vide. `mise run db` démarre Postgres puis lance ce service ;
  `mise run db-reset` efface les données puis lance `mise run db`.
- **D53** Le lot 3 crée la crate `cihub-platform` (`crates/platform/`). Elle dépend de `cihub-domain` et de
  `cihub-store` ; la dépendance vers `cihub-scm` arrive au lot 4.
- **D54** Une file est attribuée à une réplique du scheduler par un bail de la table `queue_leases` (propriétaire,
  expiration à 15 s), et chaque réconciliation prend en plus `pg_try_advisory_xact_lock` sur la file, qui garantit
  une seule réconciliation à la fois, même pendant la reprise d'un bail expiré.
- **D55** Les répliques du worker se partagent les jobs par `SELECT … FOR UPDATE SKIP LOCKED` ; `locked_until`
  rend le job d'une réplique arrêtée à une autre.
- **D56** Le lot 3 crée une table `queues` minimale (`id`, `org_id`, `repo_id`, `target_branch`, `dirty`,
  `next_tick_at`) et la clé étrangère de `events.queue_id` vers `queues`. Le lot 5 ajoute l'état, le mode et les
  réglages de la file.
- **D57** Complète D36 et D43 : le lot 3 remplace la boucle du scheduler par un job qui crée les partitions
  d'`events`. La suppression des partitions de plus de 90 jours arrive au lot 5, avec les instantanés : sans
  instantané, une file inactive perdrait son journal et son `seq` repartirait à 1. _Complétée par D67._
- **D58** Complète D49 : `jobs` et `webhook_inbox` portent `connection_id`, qui porte les quotas et le budget.
- **D59** Valeur fermée `JobClass` (`interactive`, `merge`, `speculate`, `repair`), dans `cihub-domain` et en enum
  Postgres.
- **D60** Une `idempotency_key` a la forme `<type de job>:<sujet>:<version>` (par exemple `merge_pr:item_…:1`) et
  est unique dans `jobs`. La grammaire exacte est proposée avec le premier type de job. _Complétée par D68._
- **D61** Le lot 3 n'ajoute pas de dépendance externe tant que `sqlx` suffit (`PgListener` pour `LISTEN/NOTIFY`).
- **D62** L'inbox, les jobs et l'outbox n'ont pas de trait : ce sont des fonctions de `cihub-platform` sur une
  connexion Postgres, et leurs signatures sont les interfaces figées du lot 3. Marquer une livraison comme traitée,
  écrire un événement et son entrée d'outbox, ou émettre les jobs d'une réconciliation se fait dans la transaction
  de l'appelant, qu'aucune autre implémentation ne pourrait rejoindre. Un trait arrivera avec une seconde
  implémentation, pas avant.
- **D63** Pas de NATS en v1 : Postgres est le seul transport entre les rôles. Une autre dépendance de transport
  demande une mesure et une décision.

- **D64** La boucle du scheduler, qui assemble les baux, le réveil et la transaction d'une file, arrive au lot 5
  dans `cihub-mergequeue`, avec la réconciliation. Le lot 3 livre ses pièces dans `cihub-platform`.
- **D65** Remplace D44 : l'outbox, les abonnements et le critère « un événement et son entrée d'outbox sont écrits
  dans la même transaction » passent au lot 11, le premier lot qui les consomme (webhooks sortants, ClickHouse).
- **D66** Un exécuteur est le code d'un type de job ; le worker a un registre, du type de job vers son exécuteur.
  Un exécuteur répond : terminé, à refaire plus tard (avec l'erreur), ou échec définitif (avec l'erreur). Les types
  de jobs et leurs exécuteurs sont neutres : un exécuteur appelle le SCM par le trait `Scm` (lot 4), selon la
  connexion du job ; le code propre à un SCM reste dans son adaptateur. Le contrat `Module` (lot 5) déclare ses
  exécuteurs dans ce registre.
- **D67** Complète D36 et D57 : un job quotidien `maintenance` crée les partitions d'`events` du mois et du mois
  suivant, supprime les livraisons traitées depuis plus de 7 jours et les jobs terminés depuis plus de 3 jours.
  Chaque réplique du scheduler le demande toutes les heures avec la clé du jour (`maintenance:<AAAA-MM-JJ>:1`) : la
  clé n'admet qu'un job par jour, qu'un seul worker exécute. Il remplace la boucle quotidienne des partitions.
- **D68** Complète D60 : dans une `idempotency_key`, le type est en `snake_case`, le sujet ne contient pas `:`, et
  la version est un entier à partir de 1.

### 4.1 Questions ouvertes

Chaque question porte un identifiant ; `docs/LOTS.md` indique le lot qui a besoin de la réponse. Une question
tranchée devient une décision ci-dessus et sort de cette liste.

- **Q1** La raison `tests_restarted_flake_suspected` (`docs/FLAKY_TESTS.md`) n'est pas dans le vocabulaire fermé.
  _(lot 14)_
- **Q2** `fast_forward` : méthode par défaut de la stratégie `sequential` (le commit testé atterrit) ou option,
  car elle exige un contournement de protection de branche. _(lot 7)_
- **Q3** Merge optimiste après l'éviction d'un prédécesseur : règle stricte (redémarrer l'aval) par défaut, la
  souple restant une option. _(lot 12)_
- **Q4** Push sur une PR en file : annuler (sûr, mais boucles avec Dependabot) ou resoumettre automatiquement.
  Proposition : annuler par défaut, `resubmit_on_push` comme réglage. _(lot 7)_
- **Q5** Permission optionnelle `actions: write` pour annuler la CI des branches remplacées et relancer les seuls
  jobs en échec : vaut-elle une demande d'installation plus intimidante ? _(lot 4)_
- **Q7** Conventions sémantiques OpenTelemetry pour les tests : suivre la proposition du groupe CI/CD, ou figer
  des attributs `cihub.*` tant qu'elle n'est pas stable. _(lot 14)_
- **Q8** Seuil de bascule vers ClickHouse : volume à partir duquel la documentation le recommande. _(lot 15)_
- **Q9** Stack du tableau de bord : React avec le client TypeScript généré est l'hypothèse ; à confirmer.
  _(lot 10)_
- **Q12** `docs/FLAKY_TESTS.md` abonne le module à `mergequeue.run.completed`, qui n'est pas un type d'événement
  de `docs/DOMAIN.md` (`run.transitioned` ?). _(lot 14)_
- **Q13** Un échec de merge direct donne toujours `direct_merge_rejected`, quelle que soit l'erreur du job
  (limite d'API, protection de branche…) : garder une seule raison, ou permettre les raisons précises d'échec du
  merge depuis `pending`. _(lot 7)_

---

## 5. Règles de travail

- **Pas de compatibilité avant la première version publiée** (bandeau en haut) : changer la forme et supprimer
  l'ancienne dans le même changement.
- **Proposer, puis appliquer.** Tout choix qui relève d'une décision (§4) est proposé et validé avant d'être
  écrit. En cas de doute, c'est une décision.
- **Un lot à la fois.** Prendre le lot suivant dans `docs/LOTS.md`, respecter ses interfaces figées, ne pas
  anticiper les lots suivants. Le critère de sortie automatisé est la définition de « terminé ». Dans le même
  changement que le code, mettre à jour le statut du lot et son bloc « Suivi » (D3) : ce qui est fait, ce qui
  reste, les écarts validés avec le plan.
- **Documentation.** Le code fait foi (D1). Un changement de comportement met à jour le document du sujet (§0)
  dans le même changement quand c'est possible ; sinon, le bloc « Suivi » du lot note la mise à jour qui manque.
- **Vocabulaire fermé.** Un nouvel état, une nouvelle raison ou un nouveau type d'événement se déclare dans
  `cihub-domain` (avec un commentaire `///` qui donne son sens) et dans l'enum Postgres, dans le même changement ;
  un test compare les deux énumérations. `docs/DOMAIN.md` définit le terme s'il est nouveau.
- **Noyau sans E/S.** Les crates pures reçoivent l'heure et les identifiants par paramètre, ne dépendent ni de
  `tokio`, ni de `sqlx`, ni de `reqwest`, ni d'`axum` (règles `bans` de `deny.toml`), et n'écrivent pas de logs :
  l'appelant écrit leurs décisions. Leurs tests sont des tables, des tests de propriété et des instantanés.
- **Tout appel GitHub passe par le trait `Scm`**, avec une classe de priorité. Le réconciliateur n'en fait aucun.
  Pas de client HTTP vers GitHub hors de `cihub-github`.
- **Transitions gardées en SQL** et vérification du nombre de lignes modifiées.
- **Tests** : unitaires avec `cargo nextest` ; intégration contre un vrai Postgres avec `#[sqlx::test]` (une base
  neuve par test) ; le simulateur GitHub (`cihub-github-sim`) pour tout scénario de file. Jamais de test contre le
  vrai GitHub hors du test de bout en bout de staging.
- **Tâches `mise`** (`mise.toml`) :

  | Tâche              | Contenu                                                                               | Disponible |
  | ------------------ | ------------------------------------------------------------------------------------- | ---------- |
  | `fmt`              | `cargo fmt --all`                                                                     | oui        |
  | `lint`             | `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo deny check` | oui        |
  | `test`             | `cargo nextest run --workspace`, sans les fichiers `tests/postgres.rs`                | oui        |
  | `cover`            | couverture des crates pures, 90 % au moins (D11)                                      | oui        |
  | `lint-workflows`   | `actionlint` et `zizmor` sur `.github/workflows`                                      | oui        |
  | `check`            | `lint`, puis `test`                                                                   | oui        |
  | `db`               | Postgres de `compose.yaml`, puis le rôle `cihub` par `postgres-setup` (D52)           | oui        |
  | `db-reset`         | `docker compose down --volumes`, puis `db`                                            | oui        |
  | `migrate`          | `cihub-server migrate` sur le Postgres de développement (D14)                         | oui        |
  | `test-integration` | Postgres, puis les fichiers `tests/postgres.rs` (`#[sqlx::test]`)                     | oui        |
  | `dev`              | `migrate`, puis `cihub-server` en local avec les quatre rôles, logs `pretty`          | oui        |
  | `sim`              | tests d'injection de pannes                                                           | lot 7      |

  La CI exécute `lint`, `test`, `test-integration`, `cover` et `lint-workflows`, un job par tâche (D18). Une nouvelle tâche est ajoutée
  au tableau dans le même changement.
- **Logs** : suivre les règles d'écriture de `docs/OBSERVABILITY.md`. Les lints clippy en contrôlent une partie.
- **Secrets** : jamais dans le dépôt, ni dans les tests, ni dans les journaux. La clé privée de l'App et le secret
  de webhook viennent de l'environnement ou d'un gestionnaire de secrets.

### 5.1 Pratiques Rust (D15)

- **Erreurs.** Une crate de bibliothèque déclare ses erreurs dans une énumération `thiserror`, qui garde la cause
  (`#[source]`). `anyhow` seulement dans les binaires (`cihub-server`, `cihub`), qui ajoutent le contexte pour
  l'opérateur.
- **Pas de panic implicite.** `unwrap()` et `expect()` sont interdits hors des tests (lints `unwrap_used` et
  `expect_used`). Un `expect` reste permis avec un `#[expect(clippy::expect_used, reason = "…")]` local quand
  l'invariant est prouvé.
- **Lints.** `unsafe_code = "forbid"`, `clippy::pedantic`, et `print_stdout`, `print_stderr`, `dbg_macro`, `todo`,
  `unimplemented` (`Cargo.toml`, `[workspace.lints]`). Un exemple en ligne de commande peut autoriser
  `print_stdout` et `print_stderr` pour lui seul.
- **Tests.** `proptest` pour les tests de propriété, `insta` pour les instantanés des plans, `#[sqlx::test]` pour
  l'intégration.
- **Asynchrone.** Le noyau et `plan` sont synchrones. Un calcul long passe par `spawn_blocking`. Chaque boucle
  reçoit un `CancellationToken` et s'arrête quand il est annulé. Un verrou n'est jamais gardé pendant un `.await`
  (lint `await_holding_lock`).
- **Outils.** Rust et les outils s'installent par `mise install` (D17), en binaires précompilés (D22) ; un changement de version met à jour
  `mise.lock` (`mise lock`) dans le même changement.
- **Dépendances.** Toutes les versions dans `[workspace.dependencies]` ; `default-features = false` quand c'est
  possible ; TLS par `rustls`, jamais OpenSSL.
- **Binaires.** Le profil `release` (`lto = "thin"`, `codegen-units = 1`) est ajouté avec l'image, pas avant.
- **Documentation du code.** Chaque crate commence par un commentaire `//!` qui dit son rôle et ce qu'elle ne fait
  pas. Chaque variante du vocabulaire fermé porte un commentaire `///` qui donne son sens.
