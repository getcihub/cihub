#!/usr/bin/env bash
# smoke.sh: installs the chart as a cluster deploys it and checks that CIHub
# starts: two replicas in a throwaway kind cluster, against
# a CloudNativePG Postgres 17 whose owner is the cihub role, neither superuser
# nor BYPASSRLS. It fails on a migration Job that fails, on a pod that is
# never ready or restarts, on `helm test`, and on a signed delivery that the
# web role does not accept.
#
# Needs docker, kind, kubectl, helm and openssl on PATH. `mise run smoke` runs it.
#   CIHUB_SMOKE_IMAGE    an image already built and loadable by docker, to skip
#                        the build
#   CIHUB_SMOKE_CLUSTER  the name of the kind cluster (cihub-smoke)
#   CIHUB_SMOKE_KEEP=1   keep the cluster afterwards; a run on a kept cluster
#                        keeps its Secrets and restarts the replicas
#   CIHUB_SMOKE_GITHUB=1 use the GitHub App of mise.local.toml instead of a
#                        false one, and also check that its installations are
#                        synced: a development use, never in the CI. Every
#                        variable of server.yaml that the chart carries comes
#                        from the environment then; the cluster has its own
#                        Postgres, and the chart fixes the addresses of the pod,
#                        so CIHUB_DATABASE_URL, CIHUB_HTTP_ADDR and
#                        CIHUB_INTERNAL_ADDR stay out
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

CNPG_VERSION=v1.30.1
POSTGRES_IMAGE=ghcr.io/cloudnative-pg/postgresql:17.11-minimal-trixie

cluster="${CIHUB_SMOKE_CLUSTER:-cihub-smoke}"
image="${CIHUB_SMOKE_IMAGE:-ghcr.io/getcihub/server:smoke}"
ns=cihub
export KUBECONFIG="${TMPDIR:-/tmp}/kind-${cluster}.kubeconfig"

log() { echo "==> $*"; }

github="${CIHUB_SMOKE_GITHUB:-}"
if [ "$github" = 1 ]; then
  for name in CIHUB_GITHUB_APP_ID CIHUB_GITHUB_WEBHOOK_SECRET; do
    [ -n "${!name:-}" ] || { echo "CIHUB_SMOKE_GITHUB=1 needs $name, from mise.local.toml (specs/DEPLOYMENT.md)"; exit 1; }
  done
  [ -n "${CIHUB_GITHUB_PRIVATE_KEY:-}${CIHUB_GITHUB_PRIVATE_KEY_FILE:-}" ] ||
    { echo "CIHUB_SMOKE_GITHUB=1 needs CIHUB_GITHUB_PRIVATE_KEY or CIHUB_GITHUB_PRIVATE_KEY_FILE, from mise.local.toml (specs/DEPLOYMENT.md)"; exit 1; }
  # The chart reads the client secret only with the client ID, and a pod whose
  # Secret lacks a key never starts, so the server never says which one.
  if [ -n "${CIHUB_GITHUB_CLIENT_ID:-}" ] && [ -z "${CIHUB_GITHUB_CLIENT_SECRET:-}" ]; then
    echo "CIHUB_GITHUB_CLIENT_ID needs CIHUB_GITHUB_CLIENT_SECRET, from mise.local.toml (specs/DEPLOYMENT.md)"
    exit 1
  fi
fi

# Everything worth reading when a step fails, before the cluster goes.
diagnose() {
  echo "--- pods"
  kubectl get pods -A -o wide || true
  echo "--- cihub"
  kubectl -n "$ns" describe pods -l app.kubernetes.io/name=cihub || true
  kubectl -n "$ns" logs -l app.kubernetes.io/name=cihub --all-containers --prefix --tail=200 || true
  kubectl -n "$ns" logs pod/cihub-smoke-webhook || true
  echo "--- postgres"
  kubectl -n "$ns" get cluster -o wide || true
  kubectl -n "$ns" describe cluster cihub-postgres || true
  kubectl -n "$ns" logs -l cnpg.io/cluster=cihub-postgres --tail=100 || true
  echo "--- cnpg operator"
  kubectl -n cnpg-system logs deployment/cnpg-controller-manager --tail=100 || true
}

cleanup() {
  status=$?
  if [ "$status" -ne 0 ]; then
    echo "!!! smoke test failed (exit $status)"
    diagnose
  fi
  if [ "${CIHUB_SMOKE_KEEP:-}" = 1 ]; then
    log "keeping cluster $cluster (KUBECONFIG=$KUBECONFIG)"
  else
    kind delete cluster --name "$cluster" >/dev/null 2>&1 || true
    rm -f "$KUBECONFIG"
  fi
  exit "$status"
}
trap cleanup EXIT

if [ -z "${CIHUB_SMOKE_IMAGE:-}" ]; then
  log "building $image"
  docker build -t "$image" .
fi

if ! kind get clusters 2>/dev/null | grep -qx "$cluster"; then
  log "creating kind cluster $cluster"
  kind create cluster --name "$cluster" --wait 2m
else
  log "reusing kind cluster $cluster"
  kind export kubeconfig --name "$cluster"
fi
log "loading $image"
kind load docker-image --name "$cluster" "$image"

log "installing CloudNativePG $CNPG_VERSION"
# The release manifest lives on the release branch of the minor version.
cnpg="${CNPG_VERSION#v}"
kubectl apply --server-side -f \
  "https://raw.githubusercontent.com/cloudnative-pg/cloudnative-pg/release-${cnpg%.*}/releases/cnpg-${cnpg}.yaml"
kubectl -n cnpg-system rollout status deployment/cnpg-controller-manager --timeout=3m

log "creating the Secrets"
kubectl create namespace "$ns" --dry-run=client -o yaml | kubectl apply -f -
password() { head -c 24 /dev/urandom | base64 | tr -d '/+=\n'; }
secret_exists() { kubectl -n "$ns" get secret "$1" >/dev/null 2>&1; }
# A kept cluster keeps its Secrets: Postgres has the password of the first
# run, and the replicas the webhook secret.
if ! secret_exists cihub-postgres; then
  db_password="$(password)"
  # The owner of the database, given to the Cluster as its bootstrap Secret;
  # the chart reads the URL under uri. sslmode=require: CloudNativePG serves TLS.
  kubectl -n "$ns" create secret generic cihub-postgres --type=kubernetes.io/basic-auth \
    --from-literal=username=cihub --from-literal=password="$db_password" \
    --from-literal=uri="postgres://cihub:${db_password}@cihub-postgres-rw.${ns}.svc:5432/cihub?sslmode=require"
fi
if [ "$github" = 1 ]; then
  webhook_secret="$CIHUB_GITHUB_WEBHOOK_SECRET"
  github_keys=(--from-literal=webhook-secret="$webhook_secret")
  if [ -n "${CIHUB_GITHUB_PRIVATE_KEY_FILE:-}" ]; then
    github_keys+=(--from-file=private-key.pem="$CIHUB_GITHUB_PRIVATE_KEY_FILE")
  else
    github_keys+=(--from-literal=private-key.pem="$CIHUB_GITHUB_PRIVATE_KEY")
  fi
  if [ -n "${CIHUB_GITHUB_CLIENT_ID:-}" ]; then
    github_keys+=(--from-literal=client-secret="$CIHUB_GITHUB_CLIENT_SECRET")
  fi
  kubectl -n "$ns" create secret generic cihub-github "${github_keys[@]}" \
    --dry-run=client -o yaml | kubectl apply -f -
elif secret_exists cihub-github; then
  webhook_secret="$(kubectl -n "$ns" get secret cihub-github -o go-template='{{index .data "webhook-secret"}}' | base64 --decode)"
else
  # CIHub reads the private key at startup, so it must be a key; nothing here
  # ever signs with it.
  webhook_secret="$(password)"
  kubectl -n "$ns" create secret generic cihub-github \
    --from-literal=private-key.pem="$(openssl genrsa 2048 2>/dev/null)" \
    --from-literal=webhook-secret="$webhook_secret"
fi

log "creating the Postgres cluster"
kubectl -n "$ns" apply -f - <<MANIFEST
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
metadata:
  name: cihub-postgres
spec:
  instances: 1
  imageName: ${POSTGRES_IMAGE}
  enableSuperuserAccess: false
  storage:
    size: 1Gi
  bootstrap:
    initdb:
      database: cihub
      owner: cihub
      secret:
        name: cihub-postgres
MANIFEST
kubectl -n "$ns" wait cluster/cihub-postgres --for=condition=Ready --timeout=5m

log "installing the chart"
local_values=()
if [ "$github" = 1 ]; then
  local_values=(--set github.url="${CIHUB_GITHUB_URL:-https://github.com}" --set-string github.appId="$CIHUB_GITHUB_APP_ID")
  # --set-literal, as a log filter separates its directives with commas.
  for value_and_variable in github.clientId=CIHUB_GITHUB_CLIENT_ID publicUrl=CIHUB_HTTP_PUBLIC_URL \
    log.filter=CIHUB_LOG_FILTER log.format=CIHUB_LOG_FORMAT; do
    variable="${value_and_variable#*=}"
    [ -z "${!variable:-}" ] || local_values+=(--set-literal "${value_and_variable%%=*}=${!variable}")
  done
fi
installed=""
helm status cihub -n "$ns" >/dev/null 2>&1 && installed=1
# The migration Job runs first, as a hook; --wait then returns once both
# replicas are ready: each one has read its file and reached the database.
helm upgrade --install cihub deploy/charts/cihub -n "$ns" --values scripts/smoke/values.yaml \
  --set image.repository="${image%:*}" --set image.tag="${image##*:}" \
  ${local_values[@]+"${local_values[@]}"} \
  --wait --timeout 5m
if [ -n "$installed" ]; then
  # The tag of the image does not change, so a new build or a new Secret
  # rolls nothing by itself.
  log "restarting the replicas"
  kubectl -n "$ns" rollout restart deployment/cihub
  kubectl -n "$ns" rollout status deployment/cihub --timeout=5m
fi
helm test cihub -n "$ns" --logs

log "checking that a signed delivery is accepted"
# The web role answers 202 only once it has stored the delivery. The pod
# reuses the image of the test pod, which helm test keeps.
body='{"zen":"Keep it logically awesome."}'
signature="sha256=$(printf '%s' "$body" | openssl dgst -sha256 -hmac "$webhook_secret" | awk '{print $NF}')"
curl_image="$(kubectl -n "$ns" get pod cihub-test-connection -o jsonpath='{.spec.containers[0].image}')"
kubectl -n "$ns" delete pod cihub-smoke-webhook --ignore-not-found >/dev/null
# shellcheck disable=SC2016 # the shell of the pod expands the variables
kubectl -n "$ns" run cihub-smoke-webhook --image="$curl_image" --restart=Never \
  --env=BODY="$body" --env=SIGNATURE="$signature" --command -- sh -c '
    code="$(curl -sS -o /dev/null -w "%{http_code}" -X POST \
      -H "X-GitHub-Delivery: smoke-$(date +%s)" -H "X-GitHub-Event: ping" \
      -H "X-Hub-Signature-256: $SIGNATURE" -H "Content-Type: application/json" \
      --data "$BODY" http://cihub:8080/webhooks/github)"
    echo "POST /webhooks/github with a signature: $code"
    test "$code" = 202'
for _ in $(seq 1 60); do
  phase="$(kubectl -n "$ns" get pod cihub-smoke-webhook -o jsonpath='{.status.phase}')"
  case "$phase" in Succeeded | Failed) break ;; esac
  sleep 2
done
kubectl -n "$ns" logs pod/cihub-smoke-webhook
[ "$phase" = Succeeded ] || { echo "the signed delivery was not accepted (pod $phase)"; exit 1; }

log "checking the replicas"
# Without the pods that a restart is stopping.
read -r -a pods <<<"$(kubectl -n "$ns" get pods -l app.kubernetes.io/name=cihub,app.kubernetes.io/component=server \
  -o go-template='{{range .items}}{{if not .metadata.deletionTimestamp}}{{.metadata.name}} {{end}}{{end}}')"
[ "${#pods[@]}" -eq 2 ] || { echo "expected 2 server pods, found ${#pods[@]}"; exit 1; }
for pod in "${pods[@]}"; do
  ready="$(kubectl -n "$ns" get pod "$pod" -o jsonpath='{.status.containerStatuses[0].ready}')"
  [ "$ready" = true ] || { echo "$pod is not ready"; exit 1; }
  restarts="$(kubectl -n "$ns" get pod "$pod" -o jsonpath='{.status.containerStatuses[0].restartCount}')"
  [ "$restarts" = 0 ] || { echo "$pod restarted $restarts times"; exit 1; }
done

if [ "$github" = 1 ]; then
  log "checking that the installations of the App are synced"
  # The superuser of the Postgres pod, which RLS does not filter.
  query() { kubectl -n "$ns" exec cihub-postgres-1 -c postgres -- psql -d cihub -tAc "$1"; }
  connections=0
  for _ in $(seq 1 60); do
    connections="$(query 'SELECT count(*) FROM scm_connections')"
    [ "$connections" -gt 0 ] && break
    sleep 2
  done
  [ "$connections" -gt 0 ] || { echo "no installation synced within 2 minutes: is the App installed on an account?"; exit 1; }
  query "SELECT o.slug, c.kind, c.external_id FROM scm_connections c JOIN orgs o ON o.id = c.org_id ORDER BY o.slug"
fi

log "ok: migrations applied, ${pods[*]} ready, signed delivery accepted"
if [ "$github" = 1 ] && [ "${CIHUB_SMOKE_KEEP:-}" = 1 ]; then
  cat <<NEXT
To receive the webhooks of the App:
  KUBECONFIG=$KUBECONFIG kubectl -n $ns port-forward svc/cihub 8080:8080
then a tunnel to localhost:8080, and https://<tunnel>/webhooks/github as the webhook URL of the App.
NEXT
  if [ -n "${CIHUB_GITHUB_CLIENT_ID:-}" ]; then
    echo "The sign-in sends back to ${CIHUB_HTTP_PUBLIC_URL:-<CIHUB_HTTP_PUBLIC_URL, not set>}/auth/github/callback, the Callback URL of the App,"
    echo "and an install to ${CIHUB_HTTP_PUBLIC_URL:-<CIHUB_HTTP_PUBLIC_URL, not set>}/auth/github/installed, its Setup URL."
  fi
fi
