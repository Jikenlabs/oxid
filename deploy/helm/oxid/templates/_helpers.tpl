{{/*
Expand the name of the chart.
*/}}
{{- define "oxid.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- define "oxidrender.name" -}}
{{- include "oxid.name" . }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "oxid.fullname" -}}
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
{{- define "oxidrender.fullname" -}}
{{- include "oxid.fullname" . }}
{{- end }}
