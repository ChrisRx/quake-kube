{{/*
Expand the name of the chart.
*/}}
{{- define "chart.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "chart.fullname" -}}
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
Create chart name and version as used by the chart label.
*/}}
{{- define "chart.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "chart.labels" -}}
helm.sh/chart: {{ include "chart.chart" . }}
{{ include "chart.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "chart.selectorLabels" -}}
app.kubernetes.io/name: {{ include "chart.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "chart.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "chart.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
The web app's presets, without the ones with content images when the cluster
can't mount them (gateway.imagePresets).
*/}}
{{- define "chart.presets" -}}
{{- $images := .Values.gateway.imagePresets -}}
{{- if eq (toString $images) "auto" -}}
{{- $images = semverCompare ">=1.35-0" .Capabilities.KubeVersion.Version -}}
{{- end -}}
{{- $presets := list -}}
{{- range .Values.gateway.presets -}}
{{- $preset := omit . "mapsWithoutImages" -}}
{{- $hasImages := false -}}
{{- range (dig "spec" "content" list .) -}}
{{- if .image }}{{ $hasImages = true }}{{ end -}}
{{- end -}}
{{- if or $images (not $hasImages) -}}
{{- $presets = append $presets $preset -}}
{{- else if .mapsWithoutImages -}}
{{- /* Without image volumes, the preset's maps that don't need its images. */ -}}
{{- $content := list -}}
{{- range (dig "spec" "content" list .) -}}
{{- if not .image }}{{ $content = append $content . }}{{ end -}}
{{- end -}}
{{- $spec := omit (deepCopy (default dict .spec)) "content" -}}
{{- if $content }}{{ $_ := set $spec "content" $content }}{{ end -}}
{{- $fallback := deepCopy $preset -}}
{{- $_ := set $fallback "maps" .mapsWithoutImages -}}
{{- $_ := set $fallback "spec" $spec -}}
{{- $presets = append $presets $fallback -}}
{{- end -}}
{{- end -}}
{{- toYaml $presets -}}
{{- end }}
