{{- include "cihub.required" . -}}
# The migrations run before the pods of a new version. A
# hook runs before the ConfigMap of the release exists, so the Job reads its
# configuration from the environment only.
apiVersion: batch/v1
kind: Job
metadata:
  name: {{ include "cihub.fullname" . }}-migrate
  namespace: {{ .Release.Namespace }}
  labels:
    {{- include "cihub.labels" . | nindent 4 }}
    app.kubernetes.io/component: migrate
  annotations:
    helm.sh/hook: pre-install,pre-upgrade
    # The Job stays until the next install or upgrade, for kubectl logs.
    helm.sh/hook-delete-policy: before-hook-creation
spec:
  backoffLimit: 3
  template:
    metadata:
      labels:
        {{- include "cihub.selectorLabels" . | nindent 8 }}
        app.kubernetes.io/component: migrate
    spec:
      restartPolicy: Never
      enableServiceLinks: false
      automountServiceAccountToken: false
      {{- with .Values.imagePullSecrets }}
      imagePullSecrets:
        {{- toYaml . | nindent 8 }}
      {{- end }}
      securityContext:
        {{- toYaml .Values.podSecurityContext | nindent 8 }}
      containers:
        - name: migrate
          image: {{ include "cihub.image" . | quote }}
          imagePullPolicy: {{ .Values.image.pullPolicy }}
          args:
            - migrate
            - up
          securityContext:
            {{- toYaml .Values.securityContext | nindent 12 }}
          env:
            {{- include "cihub.databaseEnv" . | nindent 12 }}
            - name: CIHUB_LOG_FILTER
              value: {{ .Values.log.filter | quote }}
            - name: CIHUB_LOG_FORMAT
              value: {{ .Values.log.format | quote }}
          {{- with .Values.resources }}
          resources:
            {{- toYaml . | nindent 12 }}
          {{- end }}
      {{- with .Values.nodeSelector }}
      nodeSelector:
        {{- toYaml . | nindent 8 }}
      {{- end }}
      {{- with .Values.affinity }}
      affinity:
        {{- toYaml . | nindent 8 }}
      {{- end }}
      {{- with .Values.tolerations }}
      tolerations:
        {{- toYaml . | nindent 8 }}
      {{- end }}
