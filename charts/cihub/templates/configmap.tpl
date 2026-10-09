{{- include "cihub.required" . -}}
# server.yaml without its secrets (specs/DEPLOYMENT.md): CIHUB_DATABASE_URL and
# CIHUB_GITHUB_WEBHOOK_SECRET come from the environment, the private key from
# a file of its Secret.
apiVersion: v1
kind: ConfigMap
metadata:
  name: {{ include "cihub.fullname" . }}
  namespace: {{ .Release.Namespace }}
  labels:
    {{- include "cihub.labels" . | nindent 4 }}
data:
  server.yaml: |
    {{- $file := dict
      "http" (dict "addr" "0.0.0.0:8080")
      "internal" (dict "addr" "0.0.0.0:8081")
      "log" (dict "filter" .Values.log.filter "format" .Values.log.format)
      "github" (dict
        "url" .Values.github.url
        "app_id" (toString .Values.github.appId)
        "private_key_file" "/etc/cihub/github/private-key.pem")
    }}
    {{- toYaml $file | nindent 4 }}
