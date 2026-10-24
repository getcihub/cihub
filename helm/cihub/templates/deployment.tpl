{{- include "cihub.required" . -}}
apiVersion: apps/v1
kind: Deployment
metadata:
  name: {{ include "cihub.fullname" . }}
  namespace: {{ .Release.Namespace }}
  labels:
    {{- include "cihub.labels" . | nindent 4 }}
    app.kubernetes.io/component: server
  {{- with .Values.deploymentAnnotations }}
  annotations:
    {{- toYaml . | nindent 4 }}
  {{- end }}
spec:
  replicas: {{ .Values.replicas }}
  {{- with .Values.strategy }}
  strategy:
    {{- toYaml . | nindent 4 }}
  {{- end }}
  selector:
    matchLabels:
      {{- include "cihub.selectorLabels" . | nindent 6 }}
      app.kubernetes.io/component: server
  template:
    metadata:
      labels:
        {{- include "cihub.labels" . | nindent 8 }}
        app.kubernetes.io/component: server
        {{- with .Values.podLabels }}
        {{- toYaml . | nindent 8 }}
        {{- end }}
      annotations:
        # cihub reads server.yaml at startup, so a change rolls the pods.
        checksum/config: {{ include (print $.Template.BasePath "/configmap.tpl") . | sha256sum }}
        {{- with .Values.podAnnotations }}
        {{- toYaml . | nindent 8 }}
        {{- end }}
    spec:
      # The legacy link variables of a Service named cihub would start with
      # CIHUB_, the prefix of the configuration.
      enableServiceLinks: false
      # CIHub does not call the Kubernetes API.
      automountServiceAccountToken: false
      {{- with .Values.imagePullSecrets }}
      imagePullSecrets:
        {{- toYaml . | nindent 8 }}
      {{- end }}
      {{- with .Values.priorityClassName }}
      priorityClassName: {{ . | quote }}
      {{- end }}
      securityContext:
        {{- toYaml .Values.podSecurityContext | nindent 8 }}
      containers:
        - name: cihub
          image: {{ include "cihub.image" . | quote }}
          imagePullPolicy: {{ .Values.image.pullPolicy }}
          args:
            - start
            - --config
            - /etc/cihub/server.yaml
          securityContext:
            {{- toYaml .Values.securityContext | nindent 12 }}
          env:
            {{- include "cihub.databaseEnv" . | nindent 12 }}
            - name: CIHUB_GITHUB_WEBHOOK_SECRET
              valueFrom:
                secretKeyRef:
                  name: {{ .Values.github.existingSecret | quote }}
                  key: {{ .Values.github.webhookSecretKey | quote }}
            {{- if .Values.github.clientId }}
            - name: CIHUB_GITHUB_CLIENT_SECRET
              valueFrom:
                secretKeyRef:
                  name: {{ .Values.github.existingSecret | quote }}
                  key: {{ .Values.github.clientSecretKey | quote }}
            - name: CIHUB_DATABASE_SECRET
              valueFrom:
                secretKeyRef:
                  name: {{ .Values.database.secretExistingSecret | default .Values.database.existingSecret | quote }}
                  key: {{ .Values.database.secretKey | quote }}
            {{- end }}
          ports:
            - name: http
              containerPort: 8080
              protocol: TCP
            - name: internal
              containerPort: 8081
              protocol: TCP
          livenessProbe:
            {{- toYaml .Values.livenessProbe | nindent 12 }}
          readinessProbe:
            {{- toYaml .Values.readinessProbe | nindent 12 }}
          {{- with .Values.startupProbe }}
          startupProbe:
            {{- toYaml . | nindent 12 }}
          {{- end }}
          {{- with .Values.resources }}
          resources:
            {{- toYaml . | nindent 12 }}
          {{- end }}
          volumeMounts:
            - name: config
              mountPath: /etc/cihub/server.yaml
              subPath: server.yaml
              readOnly: true
            - name: github
              mountPath: /etc/cihub/github
              readOnly: true
      volumes:
        - name: config
          configMap:
            name: {{ include "cihub.fullname" . }}
        - name: github
          secret:
            secretName: {{ .Values.github.existingSecret | quote }}
            items:
              - key: {{ .Values.github.privateKeyKey | quote }}
                path: private-key.pem
      {{- with .Values.nodeSelector }}
      nodeSelector:
        {{- toYaml . | nindent 8 }}
      {{- end }}
      {{- with .Values.affinity }}
      affinity:
        {{- toYaml . | nindent 8 }}
      {{- end }}
      {{- with .Values.topologySpreadConstraints }}
      topologySpreadConstraints:
        {{- toYaml . | nindent 8 }}
      {{- end }}
      {{- with .Values.tolerations }}
      tolerations:
        {{- toYaml . | nindent 8 }}
      {{- end }}
