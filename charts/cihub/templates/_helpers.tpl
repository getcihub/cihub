{{/*
Name of the chart.
*/}}
{{- define "cihub.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Full name of the resources, within the 63 characters of a DNS label.
*/}}
{{- define "cihub.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Name and version of the chart, for the chart label.
*/}}
{{- define "cihub.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Labels of every resource.
*/}}
{{- define "cihub.labels" -}}
helm.sh/chart: {{ include "cihub.chart" . }}
{{ include "cihub.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels. The server and the migration Job differ by their
app.kubernetes.io/component label.
*/}}
{{- define "cihub.selectorLabels" -}}
app.kubernetes.io/name: {{ include "cihub.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Image of CIHub: the digest wins over the tag, and the tag defaults to the
appVersion of the chart.
*/}}
{{- define "cihub.image" -}}
{{- if .Values.image.digest -}}
{{- printf "%s@%s" .Values.image.repository .Values.image.digest -}}
{{- else -}}
{{- printf "%s:%s" .Values.image.repository (.Values.image.tag | default .Chart.AppVersion) -}}
{{- end -}}
{{- end }}

{{/*
Image of the helm test pod.
*/}}
{{- define "cihub.testImage" -}}
{{- printf "%s:%s" .Values.tests.image.repository .Values.tests.image.tag -}}
{{- end }}

{{/*
The required values, checked by every template that needs them.
*/}}
{{- define "cihub.required" -}}
{{- if not .Values.database.existingSecret -}}
{{- fail "database.existingSecret is required: the Secret that holds the URL of Postgres" -}}
{{- end -}}
{{- if not (toString .Values.github.appId) -}}
{{- fail "github.appId is required: the identifier of the GitHub App" -}}
{{- end -}}
{{- if not .Values.github.existingSecret -}}
{{- fail "github.existingSecret is required: the Secret that holds the private key and the webhook secret of the GitHub App" -}}
{{- end -}}
{{- end }}

{{/*
CIHUB_DATABASE_URL, from its Secret: the server and the migration Job read
it from the environment, never from server.yaml.
*/}}
{{- define "cihub.databaseEnv" -}}
- name: CIHUB_DATABASE_URL
  valueFrom:
    secretKeyRef:
      name: {{ .Values.database.existingSecret | quote }}
      key: {{ .Values.database.key | quote }}
{{- end }}
