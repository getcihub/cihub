{{- if .Values.httpRoute.enabled -}}
{{- $route := .Values.httpRoute -}}
apiVersion: gateway.networking.k8s.io/v1
kind: HTTPRoute
metadata:
  name: {{ include "cihub.fullname" . }}
  namespace: {{ .Release.Namespace }}
  labels:
    {{- include "cihub.labels" . | nindent 4 }}
    {{- with $route.labels }}
    {{- toYaml . | nindent 4 }}
    {{- end }}
  {{- with $route.annotations }}
  annotations:
    {{- toYaml . | nindent 4 }}
  {{- end }}
spec:
  {{- with $route.parentRefs }}
  parentRefs:
    {{- toYaml . | nindent 4 }}
  {{- end }}
  {{- with $route.hostnames }}
  hostnames:
    {{- toYaml . | nindent 4 }}
  {{- end }}
  rules:
    - backendRefs:
        - name: {{ include "cihub.fullname" . }}
          port: {{ .Values.service.port }}
      {{- with $route.matches }}
      matches:
        {{- toYaml . | nindent 8 }}
      {{- end }}
{{- end }}
