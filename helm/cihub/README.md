# cihub

Self-hosted merge queue for GitHub, on Postgres

**Homepage:** <https://github.com/getcihub/cihub>

## Usage

The chart needs a Postgres 17, a GitHub App created by hand
([`specs/GITHUB.md`](https://github.com/getcihub/cihub/blob/main/specs/GITHUB.md)) and two Secrets:

```sh
kubectl create secret generic cihub-postgres \
  --from-literal=uri='postgres://cihub:<password>@<host>:5432/cihub'
kubectl create secret generic cihub-github \
  --from-file=private-key.pem=<the private key of the App> \
  --from-literal=webhook-secret=<the webhook secret of the App>
helm install cihub oci://ghcr.io/getcihub/charts/cihub \
  --set database.existingSecret=cihub-postgres \
  --set github.appId=<the App ID> \
  --set github.existingSecret=cihub-github
```

The role of the URL is neither a superuser nor `BYPASSRLS`: CIHub refuses it at startup. It owns the database, as
the migration Job uses the same URL.

## What the chart installs

- A `pre-install` and `pre-upgrade` hook Job, `cihub-server migrate`: the pods of a new version start only once
  the migrations are applied.
- A Deployment of `cihub-server start`. Each replica runs the four roles; the replicas share the work through
  Postgres. `server.yaml` comes from a ConfigMap, without secrets: the URL of the database and the webhook secret
  come from the environment, the private key from a file of its Secret.
- A Service to the public address of the pods (`POST /webhooks/github` and the API), and an optional Ingress or
  HTTPRoute. `/healthz` and `/readyz` stay on the internal address of the pods, for the probes.
- A PodDisruptionBudget when `replicas` is above 1.

The pods mount no ServiceAccount token: CIHub does not call the Kubernetes API.

## Requirements

Kubernetes: `>=1.25.0-0`

## Values

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| affinity | object | `{}` | Affinity of the pods. |
| database.connectionTimeout | int | `30` | Seconds that a query waits for a connection of the pool before it fails. |
| database.existingSecret | required | `""` | Secret that holds the URL of Postgres, `database.url` of `server.yaml`. The role is neither a superuser nor `BYPASSRLS` (CIHub refuses it at startup) and owns the database: the migration Job uses the same URL. |
| database.idleTimeout | int | `600` | Seconds after which an unused connection is closed. |
| database.key | string | `"uri"` | Key of the URL in that Secret. |
| database.poolSize | int | `10` | Largest number of connections of each replica to Postgres; the sum over the replicas stays under the `max_connections` of Postgres. |
| database.secretKey | string | `"secret"` | Key in that Secret of `database.secret`, at least 32 bytes, which seals the tokens of the people who sign in; read when `github.clientId` is set. |
| deploymentAnnotations | object | `{}` | Annotations of the Deployment. |
| fullnameOverride | string | `""` | Replaces the full name of the resources. |
| github.appId | required | `""` | Identifier of the GitHub App (specs/GITHUB.md). |
| github.clientId | string | `""` | Client ID of the App, for the sign-in to the dashboard; with it, the Secret also holds the client secret. |
| github.clientSecretKey | string | `"client-secret"` | Key of the client secret of the App in that Secret, read when `clientId` is set. |
| github.existingSecret | required | `""` | Secret that holds the private key and the webhook secret of the App. |
| github.privateKeyKey | string | `"private-key.pem"` | Key of the private key (PEM) in that Secret. |
| github.url | string | `"https://github.com"` | github.com, or the address of a GitHub Enterprise Server. |
| github.webhookSecretKey | string | `"webhook-secret"` | Key of the webhook secret in that Secret. |
| httpRoute.annotations | object | `{}` | Annotations of the HTTPRoute. |
| httpRoute.enabled | bool | `false` | Expose the Service through a Gateway API HTTPRoute, instead of an Ingress. |
| httpRoute.hostnames | list | `[]` | Host names of the route. |
| httpRoute.labels | object | `{}` | Labels of the HTTPRoute. |
| httpRoute.matches | list | `[{"path":{"type":"PathPrefix","value":"/"}}]` | Match conditions of the rule. |
| httpRoute.parentRefs | list | `[]` | Gateways and listeners of the route. |
| image.digest | string | `""` | Image digest (`sha256:…`); wins over the tag when set. |
| image.pullPolicy | string | `"IfNotPresent"` | Image pull policy. |
| image.repository | string | `"ghcr.io/getcihub/server"` | Image repository. |
| image.tag | string | `""` | Image tag; the appVersion of the chart when empty. |
| imagePullSecrets | list | `[]` | Secrets to pull the image, for the server pods, the migration Job and the test pod. |
| ingress.annotations | object | `{}` | Annotations of the Ingress. |
| ingress.className | string | `""` | IngressClass name. |
| ingress.enabled | bool | `false` | Expose the Service through an Ingress. |
| ingress.hosts | list | `[{"host":"cihub.example.com","paths":[{"path":"/","pathType":"Prefix"}]}]` | Hosts of the Ingress and their paths. |
| ingress.tls | list | `[]` | TLS configuration of the Ingress. |
| livenessProbe | object | `{"httpGet":{"path":"/healthz","port":"internal"},"periodSeconds":20}` | Liveness probe: `/healthz` answers as long as the process runs. |
| log.filter | string | `"info"` | Log filter, `log.filter` of `server.yaml` (specs/OBSERVABILITY.md). |
| log.format | string | `"json"` | Log format, `json` or `pretty`. |
| nameOverride | string | `""` | Replaces the name of the chart in the names of the resources. |
| nodeSelector | object | `{}` | Node selector of the pods. |
| podAnnotations | object | `{}` | Annotations of the server pods. |
| podDisruptionBudget.enabled | bool | `true` | Create a PodDisruptionBudget when there is more than one replica. |
| podDisruptionBudget.maxUnavailable | int | `1` | Pods that may be unavailable, as a count or a percentage; wins over `minAvailable`. |
| podDisruptionBudget.minAvailable | string | `""` | Pods that must stay available, as a count or a percentage; used when `maxUnavailable` is empty. |
| podLabels | object | `{}` | Labels of the server pods. |
| podSecurityContext | object | `{"runAsGroup":65532,"runAsNonRoot":true,"runAsUser":65532,"seccompProfile":{"type":"RuntimeDefault"}}` | Security context of the pods: the user of the image (65532), not root. |
| priorityClassName | string | `""` | Priority class of the server pods. |
| publicUrl | string | `""` | The root address where people reach the instance, such as `https://ci.example.com`. The sign-in to the dashboard sends them back there; without it, nobody signs in (specs/DEPLOYMENT.md). |
| readinessProbe | object | `{"httpGet":{"path":"/readyz","port":"internal"},"periodSeconds":10}` | Readiness probe: `/readyz` answers 503 when Postgres does not answer within 2 s. |
| replicas | int | `1` | Replicas of `cihub-server start`. Each replica runs the four roles (`web`, `scheduler`, `worker`, `notifier`), and the replicas share the work through Postgres; 2 keeps a replica that serves the webhooks while a pod or a node is lost. |
| resources | object | `{"limits":{"memory":"256Mi"},"requests":{"cpu":"50m","memory":"64Mi"}}` | Resources of the server pods and of the migration Job. |
| securityContext | object | `{"allowPrivilegeEscalation":false,"capabilities":{"drop":["ALL"]},"readOnlyRootFilesystem":true}` | Security context of the containers: CIHub writes no file. |
| service.port | int | `8080` | Port of the Service, to the public address of the pods: the webhooks on `POST /webhooks/github` and the API. The internal address (`/healthz`, `/readyz`) stays on the pods, for the probes. |
| service.type | string | `"ClusterIP"` | Type of the Service. |
| startupProbe | object | `{"failureThreshold":30,"httpGet":{"path":"/healthz","port":"internal"},"periodSeconds":2}` | Startup probe; the other probes wait until it passes. It allows a minute. |
| strategy | object | `{"rollingUpdate":{"maxSurge":1,"maxUnavailable":0},"type":"RollingUpdate"}` | Update strategy of the Deployment. A new pod is ready before an old one stops, so a replica always serves the webhooks; a published migration stays compatible with the previous version for this overlap (specs/DEPLOYMENT.md). |
| tests.image.pullPolicy | string | `"IfNotPresent"` | Pull policy of that image. |
| tests.image.repository | string | `"docker.io/curlimages/curl"` | Image of the `helm test` pod. |
| tests.image.tag | string | `"8.22.0"` | Tag of that image. |
| tolerations | list | `[]` | Tolerations of the pods. |
| topologySpreadConstraints | list | `[]` | Topology spread constraints of the server pods. |

---

_helm-docs generates this README from `Chart.yaml`, `values.yaml` and `README.md.gotmpl`; edit them, then run
`mise run helm:generate`._
