{{- include "cihub.required" . -}}
# server.yaml without its secrets (specs/DEPLOYMENT.md): CIHUB_DATABASE_URL,
# CIHUB_DATABASE_SECRET, CIHUB_GITHUB_WEBHOOK_SECRET and
# CIHUB_GITHUB_CLIENT_SECRET come from the environment, the private key from a file of its Secret.
apiVersion: v1
kind: ConfigMap
metadata:
  name: {{ include "cihub.fullname" . }}
  namespace: {{ .Release.Namespace }}
  labels:
    {{- include "cihub.labels" . | nindent 4 }}
  annotations:
    reloader.stakater.com/ignore: "true"
data:
  server.yaml: |
    {{- $http := dict "addr" "0.0.0.0:8080" }}
    {{- with .Values.publicUrl }}
    {{- $_ := set $http "public_url" . }}
    {{- end }}
    {{- with .Values.trustedProxyHops }}
    {{- $_ := set $http "trusted_proxy_hops" (int .) }}
    {{- end }}
    {{- $github := dict
      "url" .Values.github.url
      "app_id" (toString .Values.github.appId)
      "private_key_file" "/etc/cihub/github/private-key.pem" }}
    {{- with .Values.github.clientId }}
    {{- $_ := set $github "client_id" . }}
    {{- end }}
    {{- $database := dict
      "pool_size" (int .Values.database.poolSize)
      "idle_timeout" (int .Values.database.idleTimeout)
      "connection_timeout" (int .Values.database.connectionTimeout) }}
    {{- $file := dict
      "database" $database
      "http" $http
      "internal" (dict "addr" "0.0.0.0:8081")
      "log" (dict "filter" .Values.log.filter "format" .Values.log.format)
      "github" $github
    }}
    {{- $sentry := dict }}
    {{- with .Values.sentry.dsn }}
    {{- $_ := set $sentry "dsn" . }}
    {{- end }}
    {{- with .Values.sentry.browserDsn }}
    {{- $_ := set $sentry "browser_dsn" . }}
    {{- end }}
    {{- with .Values.sentry.environment }}
    {{- $_ := set $sentry "environment" . }}
    {{- end }}
    {{- with $sentry }}
    {{- $_ := set $file "sentry" . }}
    {{- end }}
    {{- toYaml $file | nindent 4 }}
