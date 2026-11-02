package server

import (
	"bytes"
	"encoding/json"
	"fmt"
	"maps"
	"os"
	"reflect"
	"regexp"
	"slices"
	"strconv"
	"strings"
	"time"

	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"sigs.k8s.io/yaml"

	"github.com/ChrisRx/quake-kube/internal/quake/content"
)

func ReadConfigFromFile(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	return ParseConfig(data)
}

// ParseConfig parses a config in the YAML the controller renders.
func ParseConfig(data []byte) (*Config, error) {
	cfg := Default()
	if err := yaml.Unmarshal(data, &cfg); err != nil {
		return nil, err
	}
	return cfg, nil
}

type Config struct {
	FragLimit int             `name:"fraglimit"`
	TimeLimit metav1.Duration `name:"timelimit"`

	BotConfig        `json:"bot"`
	GameConfig       `json:"game"`
	FileServerConfig `json:"fs"`
	ServerConfig     `json:"server"`
	Commands         []string `json:"commands"`
	// Cvars are set before the first map loads, e.g. a mod's settings.
	Cvars map[string]string `json:"cvars"`
	// Content is downloaded by q3 fetch before the server starts.
	Content []content.Source `json:"content"`
	// Generation is the QuakeServer's generation the config was rendered for. A config older
	// than the applied one is ignored, e.g. the file catching up after the gateway sent a
	// newer one.
	Generation int64 `json:"generation"`

	Maps
}

type BotConfig struct {
	MinPlayers int  `name:"bot_minplayers"`
	NoChat     bool `name:"bot_nochat"`
}

type GameConfig struct {
	ForceRespawn      bool            `name:"g_forcerespawn"`
	GameType          GameType        `name:"g_gametype"      json:"type"`
	Inactivity        metav1.Duration `name:"g_inactivity"`
	Log               string          `name:"g_log"`
	MOTD              string          `name:"g_motd"`
	Password          string          `name:"g_password"`
	QuadFactor        int             `name:"g_quadfactor"`
	SinglePlayerSkill int             `name:"g_spSkill"`
	WeaponRespawn     int             `name:"g_weaponrespawn"`
	// Native runs the mod's game code from its 32-bit x86 library rather than a QVM.
	Native bool `json:"native"`
}

type GameType int

const (
	FreeForAll     GameType = 0
	Tournament     GameType = 1
	SinglePlayer   GameType = 2
	TeamDeathmatch GameType = 3
	CaptureTheFlag GameType = 4
)

func (gt GameType) String() string {
	switch gt {
	case FreeForAll:
		return "FreeForAll"
	case Tournament:
		return "Tournament"
	case SinglePlayer:
		return "SinglePlayer"
	case TeamDeathmatch:
		return "TeamDeathmatch"
	case CaptureTheFlag:
		return "CaptureTheFlag"
	default:
		return "Unknown"
	}
}

// UnmarshalJSON accepts a game type's name, or its number for mods with game types of
// their own.
func (gt *GameType) UnmarshalJSON(data []byte) error {
	if len(data) > 0 && data[0] != '"' {
		var n int
		if err := json.Unmarshal(data, &n); err != nil {
			return err
		}
		*gt = GameType(n)
		return nil
	}
	var s string
	if err := json.Unmarshal(data, &s); err != nil {
		return err
	}
	return gt.UnmarshalText([]byte(s))
}

func (gt *GameType) UnmarshalText(data []byte) error {
	switch string(data) {
	case "FreeForAll", "FFA":
		*gt = FreeForAll
	case "Tournament":
		*gt = Tournament
	case "SinglePlayer":
		*gt = SinglePlayer
	case "TeamDeathmatch":
		*gt = TeamDeathmatch
	case "CaptureTheFlag", "CTF":
		*gt = CaptureTheFlag
	default:
		return fmt.Errorf("unknown GameType: %s", data)
	}
	return nil
}

type FileServerConfig struct {
	// allows people to base mods upon mods syntax to follow
	BaseGame string `name:"fs_basegame"`
	// set base path root C:\Program Files\Quake III Arena for files to be
	// downloaded from this path may change for TC's and MOD's
	BasePath string `name:"fs_basepath"`
	// toggle if files can be copied from servers or if client will download
	CopyFiles bool `name:"fs_copyfiles"`
	// possibly enables file server debug mode for download/uploads or
	// something
	Debug bool `name:"fs_debug"`
	// set gamedir set the game folder/dir default is baseq3
	Game string `name:"fs_game"`
	// possibly for TC's and MODS the default is the path to quake3.exe
	HomePath string `name:"fs_homepath"`
}

type ServerConfig struct {
	AllowDownload bool   `name:"sv_allowDownload"`
	DownloadURL   string `name:"sv_dlURL"`
	Hostname      string `name:"sv_hostname"`
	MaxClients    int    `name:"sv_maxclients"`
	Password      string `name:"rconpassword"`
	ListServer    string `name:"sv_master1"`
}

func (c *Config) Marshal() ([]byte, error) {
	return writeStruct(reflect.Indirect(reflect.ValueOf(c)))
}

func writeStruct(v reflect.Value) ([]byte, error) {
	if v.Kind() != reflect.Struct {
		return nil, fmt.Errorf("expected struct, received %T", v.Kind())
	}
	var b bytes.Buffer
	for i := 0; i < v.Type().NumField(); i++ {
		fv := v.Field(i)
		switch fv.Kind() {
		case reflect.Struct:
			data, err := writeStruct(fv)
			if err != nil {
				return nil, err
			}
			b.Write(data)
		case reflect.Map:
			cvars := fv.Interface().(map[string]string)
			for _, name := range slices.Sorted(maps.Keys(cvars)) {
				if !cvarName.MatchString(name) {
					return nil, fmt.Errorf("invalid cvar name %q", name)
				}
				fmt.Fprintf(&b, "seta %s %s\n", name, quote(cvars[name]))
			}
		case reflect.Slice:
			switch val := fv.Interface().(type) {
			case Maps:
				data, err := val.Marshal()
				if err != nil {
					return nil, err
				}
				b.Write(data)
			case []string, []content.Source:
			default:
				panic(fmt.Errorf("received unknown type %T", val))
			}
		default:
			tv, ok := v.Type().Field(i).Tag.Lookup("name")
			if !ok {
				continue
			}
			s := toString(v.Type().Field(i).Name, fv)
			switch tv {
			case "sv_dlURL":
				if s != "" {
					fmt.Fprintf(&b, "sets %s %s\n", tv, s)
				}
			default:
				fmt.Fprintf(&b, "seta %s %s\n", tv, quote(s))
			}
		}
	}
	for i := 0; i < v.Type().NumField(); i++ {
		if v.Type().Field(i).Name == "Commands" {
			cmds := v.Field(i).Interface().([]string)
			for _, cmd := range cmds {
				b.WriteString(cmd)
				b.WriteString("\n")
			}
		}
	}
	return b.Bytes(), nil
}

var cvarName = regexp.MustCompile(`^[A-Za-z0-9_]+$`)

// quote returns s as a quoted console argument. The console has no escapes, so characters
// that would end the argument or the command are dropped.
func quote(s string) string {
	return `"` + strings.Map(func(r rune) rune {
		if r == '"' || r < ' ' {
			return -1
		}
		return r
	}, s) + `"`
}

func toString(name string, v reflect.Value) string {
	switch val := v.Interface().(type) {
	case string:
		return val
	case int:
		return strconv.Itoa(val)
	case metav1.Duration:
		switch name {
		case "TimeLimit":
			return fmt.Sprintf("%d", int(val.Minutes()))
		default:
			return fmt.Sprintf("%d", int(val.Seconds()))
		}
	case bool:
		if val {
			return "1"
		}
		return "0"
	case GameType:
		return fmt.Sprintf("%d", val)
	case Maps:
		data, _ := val.Marshal()
		return string(data)
	default:
		panic(fmt.Errorf("received unknown type %T", v.Interface()))
	}
}

func Default() *Config {
	return &Config{
		FragLimit: 25,
		TimeLimit: metav1.Duration{Duration: 15 * time.Minute},
		Commands:  []string{},
		BotConfig: BotConfig{
			NoChat: true,
		},
		GameConfig: GameConfig{
			Log:               "",
			MOTD:              "Welcome to Quake Kube",
			QuadFactor:        3,
			GameType:          FreeForAll,
			WeaponRespawn:     3,
			Inactivity:        metav1.Duration{Duration: 10 * time.Minute},
			SinglePlayerSkill: 2,
			ForceRespawn:      false,
		},
		ServerConfig: ServerConfig{
			MaxClients: 12,
			Hostname:   "quakekube",
			Password:   "changeme",
		},
		Maps: Maps{
			{Name: "q3dm7", Type: FreeForAll},
			{Name: "q3dm17", Type: FreeForAll},
		},
	}
}

type Map struct {
	Name string   `json:"name"`
	Type GameType `json:"type"`

	CaptureLimit int             `json:"captureLimit"`
	FragLimit    int             `json:"fragLimit"`
	TimeLimit    metav1.Duration `json:"timeLimit"`
}

type Maps []Map

var mapName = regexp.MustCompile(`^[A-Za-z0-9_.+-]+$`)

func (maps Maps) Marshal() ([]byte, error) {
	var b bytes.Buffer
	for i, m := range maps {
		if !mapName.MatchString(m.Name) {
			return nil, fmt.Errorf("invalid map name %q", m.Name)
		}
		cmds := []string{
			fmt.Sprintf("g_gametype %d", m.Type),
		}
		// Mods number their game types differently, and several have more than one mode with
		// captures, so the limit applies whatever the type.
		if m.CaptureLimit != 0 {
			cmds = append(cmds, fmt.Sprintf("capturelimit %d", m.CaptureLimit))
		}
		if m.FragLimit != 0 {
			cmds = append(cmds, fmt.Sprintf("fraglimit %d", m.FragLimit))
		}
		if m.TimeLimit.Duration != 0 {
			cmds = append(
				cmds,
				fmt.Sprintf("timelimit %s", toString("TimeLimit", reflect.ValueOf(m.TimeLimit))),
			)
		}
		cmds = append(cmds, fmt.Sprintf("map %s", m.Name))
		nextmap := "d0"
		if i < len(maps)-1 {
			nextmap = fmt.Sprintf("d%d", i+1)
		}
		cmds = append(cmds, fmt.Sprintf("set nextmap vstr %s", nextmap))
		fmt.Fprintf(&b, "set d%d \"seta %s\"\n", i, strings.Join(cmds, " ; "))
	}
	b.WriteString("vstr d0\n")
	return b.Bytes(), nil
}
