package server

import (
	"fmt"
	"strings"
	"testing"

	"github.com/google/go-cmp/cmp"
	"sigs.k8s.io/yaml"
)

const config = `
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
commands:
- seta g_inactivity 600
- seta sv_timeout 120
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
`

const expectedConfig = `seta fraglimit "25"
seta bot_minplayers "0"
seta bot_nochat "0"
seta g_forcerespawn "0"
seta g_gametype "0"
seta g_log ""
seta g_motd "Welcome to Quake Kube"
seta g_password ""
seta g_quadfactor "3"
seta g_spSkill "0"
seta g_weaponrespawn "3"
seta fs_basegame ""
seta fs_basepath ""
seta fs_copyfiles "0"
seta fs_debug "0"
seta fs_game ""
seta fs_homepath ""
seta sv_allowDownload "0"
seta sv_hostname "quakekube"
seta sv_maxclients "12"
seta rconpassword "changeme"
seta sv_master1 ""
set d0 "seta g_gametype 0 ; map q3dm7 ; set nextmap vstr d1"
set d1 "seta g_gametype 0 ; map q3dm17 ; set nextmap vstr d2"
set d2 "seta g_gametype 4 ; capturelimit 8 ; map q3wctf1 ; set nextmap vstr d3"
set d3 "seta g_gametype 1 ; map q3tourney2 ; set nextmap vstr d4"
set d4 "seta g_gametype 4 ; capturelimit 8 ; map q3wctf3 ; set nextmap vstr d5"
set d5 "seta g_gametype 1 ; map ztn3tourney1 ; set nextmap vstr d0"
vstr d0
seta g_inactivity 600
seta sv_timeout 120
`

func TestConfigMarshal(t *testing.T) {
	var cfg *Config
	if err := yaml.Unmarshal([]byte(config), &cfg); err != nil {
		t.Fatal(err)
	}
	data, err := cfg.Marshal()
	if err != nil {
		t.Fatal(err)
	}
	fmt.Printf("%s\n", data)
	if diff := cmp.Diff(string(data), expectedConfig); diff != "" {
		t.Fatal(diff)
	}
}

const modConfig = `
game:
  motd: 'say "hi"; quit'
fs:
  game: q3ut4
cvars:
  g_gear: "0"
  mode_start: FFA
content:
- url: https://example.com/urt.zip
  stripComponents: 1
maps:
- name: ut4_turnpike
  type: 7
- name: ut4_casa
  type: TeamDeathmatch
`

func TestModConfigMarshal(t *testing.T) {
	var cfg *Config
	if err := yaml.Unmarshal([]byte(modConfig), &cfg); err != nil {
		t.Fatal(err)
	}
	if cfg.Game != "q3ut4" || len(cfg.Content) != 1 || cfg.Content[0].StripComponents != 1 {
		t.Fatalf("unexpected config: %+v", cfg)
	}
	data, err := cfg.Marshal()
	if err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{
		"seta g_motd \"say hi; quit\"\n",
		"seta g_gear \"0\"\nseta mode_start \"FFA\"\nset d0 ",
		"set d0 \"seta g_gametype 7 ; map ut4_turnpike ; set nextmap vstr d1\"\n",
		"set d1 \"seta g_gametype 3 ; map ut4_casa ; set nextmap vstr d0\"\n",
	} {
		if !strings.Contains(string(data), want) {
			t.Errorf("config is missing %q:\n%s", want, data)
		}
	}
}

func TestConfigRejectsUnsafeNames(t *testing.T) {
	for _, cfg := range []*Config{
		{Maps: Maps{{Name: "q3dm7 ; quit"}}},
		{Cvars: map[string]string{"a;quit": "1"}},
	} {
		if _, err := cfg.Marshal(); err == nil {
			t.Errorf("expected an error for %+v", cfg)
		}
	}
}
