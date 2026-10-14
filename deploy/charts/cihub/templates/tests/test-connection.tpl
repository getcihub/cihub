apiVersion: v1
kind: Pod
metadata:
  name: {{ include "cihub.fullname" . }}-test-connection
  namespace: {{ .Release.Namespace }}
  labels:
    {{- include "cihub.labels" . | nindent 4 }}
  annotations:
    helm.sh/hook: test
    # Without hook-succeeded, helm test does not wait for Helm 4 to delete the
    # pod after a green run; a failed run keeps its pod for helm test --logs.
    helm.sh/hook-delete-policy: before-hook-creation
spec:
  restartPolicy: Never
  automountServiceAccountToken: false
  {{- with .Values.imagePullSecrets }}
  imagePullSecrets:
    {{- toYaml . | nindent 4 }}
  {{- end }}
  securityContext:
    runAsNonRoot: true
    runAsUser: 65532
    runAsGroup: 65532
    seccompProfile:
      type: RuntimeDefault
  containers:
    - name: connection
      image: {{ include "cihub.testImage" . | quote }}
      imagePullPolicy: {{ .Values.tests.image.pullPolicy }}
      securityContext:
        allowPrivilegeEscalation: false
        readOnlyRootFilesystem: true
        capabilities:
          drop:
            - ALL
      # The web role refuses an unsigned delivery with 401: the Service
      # routes to a replica whose public address serves the webhooks.
      command:
        - sh
        - -c
        - |
          code="$(curl -sS -o /dev/null -w '%{http_code}' -X POST http://{{ include "cihub.fullname" . }}:{{ .Values.service.port }}/webhooks/github)"
          echo "POST /webhooks/github without a signature: $code"
          test "$code" = 401
