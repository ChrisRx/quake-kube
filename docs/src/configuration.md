# Configuration

The server and maps are configured via ConfigMap that is mounted to the container:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: quake3-server-config
data:
  config.yaml: |
    fragLimit: 25
    timeLimit: 15m
    game:
      motd: "Welcome to Quake Kube"
      type: FreeForAll
      forceRespawn: false
      inactivity: 10m
      quadFactor: 3
      weaponRespawn: 3
    server:
      hostname: "quakekube"
      maxClients: 12
      password: "changeme"
    maps:
    - name: q3dm7
      type: FreeForAll
    - name: q3dm17
      type: FreeForAll
    - name: q3wctf1
      type: CaptureTheFlag
      captureLimit: 8
    - name: q3tourney2
      type: Tournament
    - name: q3wctf3
      type: CaptureTheFlag
      captureLimit: 8
    - name: ztn3tourney1
      type: Tournament
```

The time limit and frag limit can be specified with each map (it will change it for subsequent maps in the list):

```yaml
- name: q3dm17
  type: FreeForAll
  fragLimit: 30
  timeLimit: 30
```

Capture limit for CTF maps can also be configured:

```yaml
- name: q3wctf3
  type: CaptureTheFlag
  captureLimit: 8
```

Any commands not captured by the config yaml can be specified in the `commands` section:

```yaml
commands:
- seta g_inactivity 600
- seta sv_timeout 120
```
