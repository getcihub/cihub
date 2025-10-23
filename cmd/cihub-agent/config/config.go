package config

import (
	"fmt"
	"os"
	"strings"

	"github.com/knadh/koanf/parsers/yaml"
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
	// Config provides the system configuration.
	Config struct {
		Agent  Agent  `koanf:"agent"`
		Logger Logger `koanf:"logger"`
		RPC    RPC    `koanf:"rpc"`
	}

	// Agent provides the agent configuration.
	Agent struct {
		Enabled     bool   `koanf:"enabled"`
		Name        string `koanf:"name"`
		Containerd  string `koanf:"containerd"`
		Firecracker string `koanf:"firecracker"`
		Kernel      Kernel `koanf:"kernel"`
		Snapshotter string `koanf:"snapshotter"`
		Pools       []Pool `koanf:"pools"`
	}

	// Kernel provides the kernel configuration to use for an agent
	Kernel struct {
		Args string `koanf:"args"`
		Path string `koanf:"path"`
	}

	// Pool provides a pool of runner configuration
	Pool struct {
		ID       string   `koanf:"id"`
		Capacity int      `koanf:"capacity"`
		Labels   []string `koanf:"labels"`
		Memory   int64    `koanf:"memory"`
		OS       string   `koanf:"os"`
		VCPU     int64    `koanf:"vcpu"`
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

	// Load configuration from specified YAML file first
	err := k.Load(file.Provider(path), yaml.Parser())
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
	if len(config.Agent.Pools) == 0 {
		return Config{}, fmt.Errorf("config: must provide at least one poll configuration")
	}

	return config, nil
}
