package config

import (
	"fmt"
	"os"
	"strings"

	"github.com/knadh/koanf/parsers/toml"
	"github.com/knadh/koanf/providers/env/v2"
	"github.com/knadh/koanf/providers/file"
	"github.com/knadh/koanf/v2"
	"github.com/sirupsen/logrus"
)

// default agent hostname.
var hostname string

func init() {
	hostname, _ = os.Hostname()
	if hostname == "" {
		hostname = "localhost"
	}
}

type (
	// Config provides the agent configuration.
	Config struct {
		Agent  Agent   `koanf:"agent"`
		Labels []Label `koanf:"labels"`
		Logger Logger  `koanf:"logger"`
		RPC    RPC     `koanf:"rpc"`
	}

	Agent struct {
		Enabled     bool     `koanf:"enabled"`
		Name        string   `koanf:"name"`
		Capacity    int      `koanf:"capacity"`
		Containerd  string   `koanf:"containerd"`
		Kernel      string   `koanf:"kernel"`
		Labels      []string `koanf:"labels"`
		Memory      int64    `koanf:"memory"`
		OS          string   `koanf:"os"`
		Snapshotter string   `koanf:"snapshotter"`
		VCPU        int64    `koanf:"vcpu"`
	}

	// Label defines resource specifications for runners loaded from configuration.
	Label struct {
		Name    string `koanf:"name"`
		CPU     int    `koanf:"cpu"`
		RAM     int    `koanf:"ram"`
		Storage int    `koanf:"storage"`
		Kernel  string `koanf:"kernel"`
		Ubuntu  string `koanf:"ubuntu"`
	}

	// Logger provides the logger configuration.
	Logger struct {
		Level logrus.Level `koanf:"level"`
	}

	// RPC provides the RPC server configuration.
	RPC struct {
		Host   string `koanf:"host"`
		Proto  string `koanf:"proto"`
		Secret string `koanf:"secret"`
	}
)

// Load loads the configuration from a file and environment variable.
func Load(path string) (Config, error) {
	k := koanf.New(".")

	// Load configuration from specified TOML file first
	err := k.Load(file.Provider(path), toml.Parser())
	if err != nil {
		return Config{}, fmt.Errorf("config: failed to load configuration file at '%s': %w", path, err)
	}

	// Load configuration from environment variable
	//
	// Environment variables must have prefix "CIHUB_"
	k.Load(env.Provider(".", env.Opt{
		Prefix: "CIHUB_",
		TransformFunc: func(k, v string) (string, any) {
			k = strings.ReplaceAll(strings.ToLower(strings.TrimPrefix(k, "CIHUB_")), "_", ".")
			if strings.Contains(v, " ") {
				return k, strings.Split(v, " ")
			}
			return k, v
		},
	}), nil)

	var config Config
	if err := k.Unmarshal("", &config); err != nil {
		return Config{}, fmt.Errorf("config: failed to read configuration file: %w", err)
	}

	if config.Agent.Name == "" {
		config.Agent.Name = hostname
	}

	return config, nil
}
