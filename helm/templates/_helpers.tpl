{{- define "controlPlane.name" -}}
{{- default .Values.controlPlane.name "kubera-gateway-control-plane" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "gateway_class.name" -}}
{{- default .Values.gatewayClass.name "kubera-gateway" | trunc 63 | trimSuffix "-" }}
{{- end }}


{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "labels" -}}
helm.sh/chart: {{ include "chart" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}
