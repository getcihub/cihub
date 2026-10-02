# Configuration

Contexte : qui ajoute ou modifie un réglage de file, une stratégie ou une clé du fichier de configuration du dépôt.
Le code est la source de vérité (`AGENTS.md` D1) : le modèle du fichier est dans `crates/config/src/file.rs`, les
réglages, leurs valeurs par défaut, leurs plages et la table des stratégies dans `crates/config/src/settings.rs`.
Ce fichier les explique.

Le comportement d'une file se configure dans un fichier `.cihub.yaml` ou `.cihub.yml` à la racine du dépôt, lu sur
la tête de la branche cible (pour qu'une PR ne puisse pas assouplir sa propre barrière), ou dans le tableau de
bord, qui écrit le même modèle. Si les deux noms de fichier existent, la configuration est en erreur, car aucun des
deux ne l'emporte. Plusieurs files peuvent viser la même branche : une PR est routée vers la première file, dans
l'ordre du fichier, dont les `queue_conditions` sont satisfaites, ou vers celle nommée dans la commande
(`/cihub merge --queue hotfix`).

```yaml
version: 1
queues:
  - name: main
    branch: main
    strategy: batched                     # sequential | pipelined | batched | parallel
    merge_method: squash                  # squash | merge_commit | rebase | fast_forward
    trigger: draft_pr                     # draft_pr | push
    queue_conditions:                     # admission : la PR elle-meme
      - github-review-decision = approved
      - -draft
      - -label = do-not-merge
      - check-success = lint
    merge_conditions:                     # sur le commit speculatif
      - check-success = build
      - check-success = integration-tests
    window: { floor: 2, ceiling: 20 }
    batching: { size: 8, max_wait_minutes: 5 }
    failure_budget: { retries: 1 }
    pending_failure_depth: 1
    optimistic_merging: true
  - name: hotfix
    branch: main
    strategy: sequential
    merge_method: fast_forward
    queue_conditions:
      - label = hotfix
      - sender-permission >= write
priority_rules:
  - name: urgent
    conditions:
      - label = urgent
    priority: urgent                      # urgent | high | medium | low | 0-255
  - name: dependabot
    conditions:
      - author = dependabot[bot]          # une liste en ligne coupe le YAML sur les crochets
    priority: low
defaults:                                 # tout reglage d'une file ; chaque file peut le remplacer
  label_commands: { enqueue_label: ready-to-merge }
  merge_commit_title: pr_title            # pr_title | pr_title_and_number
```

## Erreurs

Le fichier doit commencer par `version: 1` : une autre valeur, ou son absence, donne
`start_test_run_config_bad_version`. Une clé inconnue, une valeur hors plage ou deux réglages qui ne vont pas
ensemble donnent `start_test_run_config_parsing_failure`, avec la ligne et la colonne de la valeur fautive. Une
liste `required_statuses` avec un nom vide ou en double donne `start_test_run_config_bad_required_statuses`.
`defaults` accepte tous les réglages d'une file, mais pas `name`, `branch` ni les conditions ; deux files ne
peuvent pas avoir le même `name`.

## Sources et stratégies

Chaque réglage prend la valeur de la source la plus forte, dans l'ordre `default` < `strategy` < `file`
(`defaults`, puis la file) < `ui`. `cihub-config` rend chaque valeur avec sa source, que l'interface affiche pour
signaler un écart avec la stratégie. Les réglages individuels sont modifiables par les admins de l'organisation
(avec journal d'audit) et lisibles par les membres. L'état de la file (`running`, `paused`, `draining`) n'est pas un
réglage du fichier : seuls le tableau de bord, l'API et la CLI le changent.

Le premier réglage d'une file est `strategy` (`pipelined` par défaut), qui positionne les valeurs de ceux qui
suivent. Une stratégie est une table de valeurs par défaut, sans code dans le noyau :

| `strategy`   | `mode`     | Fenêtre (plancher / plafond) | Lots               | `pending_failure_depth` | `optimistic_merging` |
| ------------ | ---------- | ---------------------------- | ------------------ | ----------------------- | -------------------- |
| `sequential` | `single`   | 1 / 1                        | non                | 0                       | non                  |
| `pipelined`  | `single`   | 2 / 20                       | non                | 0                       | non                  |
| `batched`    | `single`   | 2 / 20                       | oui, 8 au plus     | 1                       | oui                  |
| `parallel`   | `parallel` | 2 / 20                       | oui, 4 au plus     | 1                       | oui                  |

Toutes les stratégies activent le merge direct (`direct_merge_mode: always`) et une relance du budget d'échec.

## Réglages

| Réglage                                                        | Défaut                              | Plage                                                                      |
| -------------------------------------------------------------- | ----------------------------------- | -------------------------------------------------------------------------- |
| `mode`                                                         | `single`                            | `single`, `parallel`                                                       |
| `window.floor` / `window.ceiling`                              | 2 / 20                              | 1–100, plancher ≤ plafond                                                  |
| `testing_concurrency`                                          | aucun (raccourci)                   | 1–100 ; fixe plancher et plafond à la même valeur ; exclu avec `window`    |
| `bisection_concurrency`                                        | le plafond de la fenêtre            | 1–200                                                                      |
| `testing_timeout_minutes`                                      | 300                                 | 10–1440                                                                    |
| `not_ready_timeout_hours`                                      | 168                                 | 1–720                                                                      |
| `merge_method`                                                 | `squash`                            | `squash`, `merge_commit`, `rebase`, `fast_forward` (stratégie `sequential` seulement, `AGENTS.md` D120) |
| `merge_commit_title`                                           | `pr_title`                          | `pr_title`, `pr_title_and_number`                                          |
| `trigger`                                                      | `draft_pr`                          | `draft_pr`, `push` ; `inplace` arrive avec un lot ultérieur et est refusé d'ici là (`AGENTS.md` D119) |
| `update_method`                                                | `merge`                             | `merge` ; utilisé par `inplace` (`rebase` arrive avec un lot ultérieur, `AGENTS.md` D114) |
| `batching.enabled` / `size` / `max_wait_minutes`               | désactivé / 4 / 5                   | taille maximale 2–20, attente 1–60                                         |
| `failure_budget.retries`                                       | 1                                   | 0–10                                                                       |
| `optimistic_merging`                                           | désactivé                           | booléen                                                                    |
| `pending_failure_depth`                                        | 0                                   | 0–10                                                                       |
| `direct_merge_mode`                                            | `always`                            | `off`, `always`                                                            |
| `optimization_mode`                                            | `bisection_skip_redundant_tests`    | `off`, `bisection_skip_redundant_tests`                                    |
| `comments_enabled`, `commands_enabled`, `status_check_enabled` | activés                             | booléen                                                                    |
| `label_commands.enabled` / `enqueue_label`                     | activé / `cihub-merge-queue-submit` | chaîne non vide                                                            |
| `state_labels_enabled`                                         | désactivé                           | booléen                                                                    |
| `keep_test_branches_minutes`                                   | 0                                   | 0–1440                                                                     |
| `branch_prefix`                                                | `cihub`                             | chaîne sans `/` ni espace ; branches `<prefix>-merge/` et `<prefix>-temp/` |
| `branch_protection_injection`                                  | `both`                              | `both`, `merge`, `none`                                                    |
| `required_statuses`                                            | hérités de la protection de branche | liste ; liste vide = aucun, `null` = héritage                              |

## Options par PR et files multiples

Les options par PR sont posées à la soumission et stockées sur l'entrée : priorité, `no_batch`, `direct` (admins,
nécessite l'App sudo). Les priorités ont les mêmes noms et valeurs que chez Trunk (`urgent` 0, `high` 10, `medium`
100, `low` 200), pour que les mêmes commentaires fonctionnent. Un dépôt peut avoir plusieurs files (`main`,
branches `release/*`), chacune avec ses propres réglages ; une PR est routée par sa branche de base, et un
changement de branche de base annule l'entrée avec `pr_base_branch_changed`.

Une file ne peut pas être supprimée tant qu'elle contient des entrées vivantes ; sa suppression efface ses réglages
et son historique de Postgres mais pas de ClickHouse. Changer de `mode` avec des entrées en vol fait passer en
`switching_modes`, refuse les nouvelles soumissions avec `queue_switching_modes`, et se termine automatiquement
quand la dernière entrée se conclut.
