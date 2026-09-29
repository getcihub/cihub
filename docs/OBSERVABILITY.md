# Observabilité

Contexte : qui ajoute une métrique, un log de décision, une trace ou une alerte. Le code est la source de vérité (`AGENTS.md` D1) ; les règles d'écriture des logs sont ici.

Chaque processus émet des traces OpenTelemetry, des métriques Prometheus et des logs structurés avec les mêmes clés de corrélation (`org_id`, `queue_id`, `item_id`, `run_id`, `delivery_id`) : une question comme « pourquoi la PR 4821 a-t-elle redémarré à 14 h 03 » se répond d'abord par la chronologie d'événements de l'entrée, ensuite par les traces. La chronologie est l'outil de débogage principal et elle est exposée aux clients ; le reste sert à exploiter le service.

| Signal                  | Contenu                                                                                                                                                                                                                                                                                                                                                                                                  | Où                                                                                                                                                                          |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Chronologie de l'entrée | Chaque transition avec sa raison, son acteur et son payload                                                                                                                                                                                                                                                                                                                                              | `events` Postgres (90 jours), tables de faits ou ClickHouse (13 mois), page PR du tableau de bord, API `?include=timeline`                                                  |
| Métriques de service    | `cihub_reconcile_duration_seconds{queue}`, `cihub_reconcile_errors_total`, `cihub_jobs_inflight{type}`, `cihub_job_duration_seconds{type}`, `cihub_job_failures_total{type,reason}`, `cihub_webhook_inbox_lag_seconds`, `cihub_outbox_lag_seconds`, `cihub_github_requests_total{installation,endpoint,status}`, `cihub_github_rate_limit_remaining{installation}`, `cihub_clickhouse_insert_batch_size` | Prometheus, collectées sur `/metrics` de l'adresse interne de chaque processus, anonyme par défaut ou avec authentification (compte machine ou admin, `docs/SECURITY.md`) |
| Métriques produit       | Les séries `mq_*` par file (profondeur, attente de mergeabilité, slots, conclusions, redémarrages, histogrammes d'attente, de test et de temps en file)                                                                                                                                                                                                                                                  | `GET /api/v1/metrics` côté client, et les mêmes séries dans le Prometheus interne avec un label `org` pour les vues de flotte                                                   |
| Traces                  | Une trace par livraison de webhook, de l'inbox à la réconciliation puis aux jobs et aux appels GitHub ; une par requête d'API                                                                                                                                                                                                                                                                            | Collecteur OpenTelemetry, échantillonnage 10 % plus 100 % des erreurs                                                                                                       |
| Logs                    | Structurés, une ligne par décision de réconciliation (`decision=start_run group=... predecessors=...`) et par étape de job                                                                                                                                                                                                                                                                               | Loki ou le stockage de logs du cloud, 30 jours                                                                                                                              |

## Logs

Les logs utilisent `tracing` et `tracing-subscriber` (`AGENTS.md` D13). Seul `cihub-server` (et la CLI pour ses
propres messages) installe le subscriber, par `cihub-telemetry` (`AGENTS.md` D34) ; les crates de bibliothèque écrivent avec les macros de `tracing`
(`info!`, `warn!`…) et ouvrent des spans. Les crates pures n'écrivent pas de logs : l'appelant écrit leurs
décisions.

### Réglages

| Variable            | Défaut | Rôle                                                                                                                              |
| ------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------- |
| `CIHUB_LOG`         | `info` | Filtre de niveau, global ou par crate, à la syntaxe de `EnvFilter` : `info,cihub_github=debug` passe une seule crate en `debug` |
| `CIHUB_LOG_FORMAT`  | `json` | `json` (une ligne JSON par enregistrement, pour le stockage de logs) ou `pretty` (une ligne de texte en couleur, pour un terminal)    |

Une valeur invalide arrête le processus au démarrage. `mise run dev` utilise `pretty`.

### Format

En `json`, un enregistrement est une ligne. Les clés de corrélation des spans ouverts sont au premier niveau, à côté
des champs de l'événement, pour qu'une requête sur `queue_id` trouve toutes les lignes d'une file :

```json
{"timestamp":"2026-09-29T14:03:12.481Z","level":"INFO","target":"cihub_mergequeue::reconcile","message":"group formed","org_id":"org_0627ank54ajth78mw4353dqpw5dvt5kp","queue_id":"queue_05ywjdspm06ftp1ahh11w4vk086z2kjk","group_id":"group_…","predecessors":2}
```

`timestamp` est en RFC 3339 UTC, `level` vaut `DEBUG`, `INFO`, `WARN` ou `ERROR`, `target` est le module Rust qui
écrit. Le format JSON standard de `tracing-subscriber` place les champs des spans dans une liste `spans` et non au
premier niveau : `cihub-telemetry` les met à plat avec son propre formateur (`crates/telemetry/src/logs.rs`), sans
autre dépendance. Les clés d'une ligne sont dans l'ordre alphabétique. Les `trace_id` et `span_id` s'ajoutent quand les traces OpenTelemetry arrivent.

### Clés de corrélation

Chaque unité de travail ouvre un span qui porte ses clés ; toutes les lignes écrites à l'intérieur en héritent.

| Span        | Ouvert par                                    | Clés                              |
| ----------- | --------------------------------------------- | --------------------------------- |
| `webhook`   | réception d'un webhook (rôle `api`)           | `delivery_id`                     |
| `reconcile` | réconciliation d'une file (rôle `scheduler`)  | `org_id`, `queue_id`              |
| `job`       | exécution d'un job (rôle `worker`)            | `job_type`, `job_id`, et `run_id` quand le job concerne une exécution |
| `request`   | requête de l'API (rôle `api`)                 | `request_id`, puis `org_id` après l'authentification |

Chaque processus ouvre aussi un span par rôle, avec la clé `role`. Les noms de clés ci-dessus sont les seuls
permis pour ces données (`org_id`, `queue_id`, `item_id`, `group_id`, `run_id`, `delivery_id`, `job_id`,
`job_type`, `request_id`, `role`) ; les mêmes noms servent d'attributs de span et de labels de métriques.

### Règles d'écriture des logs

Ces règles s'appliquent à tout le code, écrit par un humain ou par un agent. Les lints clippy vérifient celles qui
portent la mention _lint_ ; la relecture vérifie les autres.

1. **Pas de sortie directe.** Ni `println!`, ni `eprintln!`, ni `dbg!` hors des exemples en ligne de commande et
   de la sortie normale de la CLI. _(lint : `print_stdout`, `print_stderr`, `dbg_macro`)_
2. **Les clés de corrélation s'ajoutent une fois, au début de l'unité de travail**, dans son span
   (`#[instrument(skip_all, fields(queue_id = %queue_id))]` ou `info_span!`). Le code appelé ne les répète pas.
3. **Le message est une constante**, en minuscules, qui nomme ce qui s'est passé (`"merge refused"`,
   `"job step done"`). Les données variables vont dans les champs : `info!(pr_number, "item submitted")`, jamais
   `info!("item {pr_number} submitted")`. Une requête sur `message` trouve ainsi toutes les lignes du même type.
4. **Les champs sont en snake_case.** Une nouvelle clé partagée entre crates est ajoutée à la liste ci-dessus.
5. **Une erreur utilise le champ `error`** et s'écrit une seule fois, là où elle est traitée :
   `warn!(error = &err as &dyn std::error::Error, "github call retried")`, pour garder la chaîne des causes. Une
   fonction qui renvoie l'erreur l'enveloppe (`#[source]`) et ne l'écrit pas.
6. **Une durée est un entier suffixé par son unité** (`duration_ms`), pas un `Duration` ni une chaîne.
7. **Le niveau dit qui doit lire la ligne.**

   | Niveau  | Quand                                             | Exemples                                                                                                                                                    |
   | ------- | ------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
   | `DEBUG` | Détail pour un développeur ; éteint en production | Chaque appel GitHub et son coût, chaque requête réessayée, l'écart entre le plan et l'état observé                                                          |
   | `INFO`  | Une ligne par décision ou changement d'état       | Décision de réconciliation (`decision=start_run group=... predecessors=...`), étape de job, webhook stocké, migration appliquée, changement de mode dégradé |
   | `WARN`  | Un problème que CIHub traite seul                 | Nouvel essai après une erreur GitHub, budget en mode `throttled`, GitHub indisponible, bail de file perdu                                                   |
   | `ERROR` | Un opérateur doit agir                            | Job envoyé en file de rebut, invariant violé, migration en échec                                                                                            |

   Une CI rouge, une PR éjectée de la file ou un webhook en double sont des résultats normaux : `INFO`, jamais
   `ERROR`.

8. **Aucun secret, aucune donnée de tenant brute.** Pas de jeton, de clé, de secret de webhook, de payload de
   webhook, de diff ni de corps de requête GitHub : on écrit l'identifiant (`delivery_id`, `run_id`). Un secret est
   un `secrecy::SecretString`, dont l'affichage est masqué.
9. **Un champ coûteux se calcule seulement quand il est écrit** : l'appel est gardé par
   `tracing::enabled!(Level::DEBUG)`, ou le champ est une valeur qui se formate paresseusement.
10. **Les logs ne remplacent pas la chronologie.** La chronologie d'événements dit pourquoi une entrée a changé
    d'état ; les logs disent ce que le processus a fait. Une information dont un client a besoin va dans un
    événement avec sa raison (`docs/DOMAIN.md`), jamais seulement dans un log.
11. **Les tests n'écrivent pas sur la sortie standard.** Un test qui vérifie un log installe un subscriber local
    (`tracing::subscriber::with_default`) qui écrit dans un tampon.

## Alertes et tableaux de bord

Alertes qui réveillent : plus de 1 % d'erreurs de réconciliation pendant 5 min ; retard de l'inbox webhook au-delà de 60 s ; une file dont une exécution dépasse `deadline_at` de plus de 10 min et reste en cours (timeout bloqué) ; retard d'outbox au-delà de 10 min ; budget de rate limit GitHub sous 10 % pour une installation ayant des entrées vivantes ; file de rebut des jobs non vide. Alertes en simple notification : échecs d'insertion ClickHouse, endpoints webhook clients désactivés automatiquement, installation d'App suspendue.

Tableaux de bord d'exploitation : vue de flotte (files par état, entrées par état, exécutions en cours, merges par heure), budget GitHub par installation, débit des jobs et répartition des erreurs par type et raison, et une vue « files les plus lentes » triée par temps en file p95 sur la dernière heure. Les clients voient la vue Santé (conclusions, temps en file, durée de test, redémarrages par cause) et le graphe en direct.
