# GOAL — à quoi sert CIHub

Contexte : pour tout le monde (humains et agents) qui travaille sur ce dépôt. À lire avant `AGENTS.md`. Cette
page dit **ce qu'on veut** ; `AGENTS.md` dit **comment on le construit**. Quand un choix n'est pas évident,
revenir ici et se demander : quelle option sert le mieux cet objectif ?

## La phrase

**Une file de merge pour GitHub, open source et auto-hébergée, qui garde la branche cible toujours verte avec
n'importe quelle CI, s'installe en moins de trente minutes avec une image et un Postgres, et explique chacune de
ses décisions.**

## Ce que cela veut dire

1. **La branche cible reste verte.** Une PR ne merge qu'après que ses tests sont passés sur un commit qui contient
   toutes les PR devant elle. Les statuts requis sont lus sur la branche cible, jamais depuis la PR, pour qu'une PR
   ne puisse pas affaiblir sa propre barrière.
2. **Une file efficace, pas une file unique.** Plusieurs stratégies choisies par file (séquentielle, prédictive,
   par lots, parallèle par cibles impactées), une fenêtre de spéculation adaptative, un budget d'échec qui ne
   retire jamais une PR sur un seul échec, et quatre méthodes de merge. Les stratégies sont des tables de valeurs
   par défaut, pas du code dans le noyau.
3. **Agnostique de la CI.** CIHub ne lance jamais la CI : il pousse une branche de test et attend les statuts et
   check runs requis. GitHub Actions, Buildkite, CircleCI, Jenkins fonctionnent sans modification.
4. **Simple à installer et à exploiter.** Une image, un Postgres 17, une GitHub App créée selon `docs/GITHUB.md` ;
   `docker compose up` suffit pour une organisation. Moins de trente minutes de la première commande à la
   première PR mergée.
5. **Chaque décision s'explique.** Chaque transition porte une raison d'un vocabulaire fermé ; un journal
   d'événements par PR répond à « pourquoi ma PR attend-elle encore » et alimente les tableaux de bord.
6. **Aucune intention n'est perdue.** Une soumission, une commande ou un résultat de check survit aux crashes,
   aux webhooks perdus, aux pannes et au rate limit de GitHub : au pire il est retardé, jamais oublié.
7. **Les tests instables ne rejettent pas les PR.** Le module `flakytests` garde l'historique des résultats (OTLP
   natif, JUnit converti) et la file ne désigne pas un coupable sur un test connu comme instable.
8. **Une migration facile.** Labels, commandes (`/trunk merge`), priorités, webhooks et fichier de configuration
   de Trunk, Mergify ou de la file native GitHub sont convertibles, sans imposer leur vocabulaire.
9. **Multi-tenant par construction.** Une instance sert plusieurs organisations GitHub, ou un hébergeur tiers ;
   une organisation bruyante ralentit ses propres files, pas celles des autres.
10. **Rester petit.** Postgres est la seule dépendance en v1 : pas de broker, pas de cache externe. ClickHouse
    arrive derrière une interface quand une mesure le justifie.

## Comment on sait qu'on y est (acceptation)

| Affirmation       | Barre                                                                                                                                                                                                      |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Installation      | Une personne extérieure installe CIHub et merge une première PR en moins de 30 minutes en suivant le README                                                                                                |
| Branche verte     | Le dépôt de CIHub merge par CIHub pendant 2 semaines sans merge rouge                                                                                                                                      |
| Résilience        | Sous le simulateur GitHub (5xx, 429, webhooks perdus, dupliqués, désordonnés, processus tués), chaque PR verte est mergée exactement une fois, dans l'ordre du graphe, et chaque PR rouge porte une raison |
| Webhook perdu     | Coûte au plus un intervalle de la boucle de réparation, jamais un blocage                                                                                                                                  |
| Explicabilité     | Chaque sortie de file a une raison du vocabulaire fermé et une page                                                                                                                                        |
| Budget GitHub     | Un dépôt de 50 PR en attente reste sous 300 points GraphQL par heure                                                                                                                                       |
| Débit (`batched`) | Temps en file p50 réduit de 40 % sur une charge synthétique de 20 PR par heure ; la bissection isole un échec en ≤ log2(taille du lot) exécutions                                                          |
| Tests instables   | Sur un dépôt avec un test instable connu, zéro PR évincée à tort                                                                                                                                           |
| Parallèle         | Un monodépôt pilote merge des PR disjointes en parallèle                                                                                                                                                   |
| Échelle           | Une instance tient 1 000 PR par jour avec ClickHouse activé                                                                                                                                                |

## Ce qu'on n'optimise **pas** (hors périmètre de la v1)

- Héberger ou lancer la CI.
- La revue de code, une extension de navigateur, une app Slack au-delà des webhooks sortants, un provider
  Terraform.
- GitLab et Bitbucket : le trait `Scm` les prévoit, la v1 ne les implémente pas.
- ClickHouse lui-même : il arrive quand le volume le justifie, derrière le trait `AnalyticsStore`.
- La spéculation probabiliste à la Uber : hors v1, mais le journal garde dès maintenant les attributs utiles (`AGENTS.md` D27).
