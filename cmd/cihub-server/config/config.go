package config

import (
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/knadh/koanf/parsers/toml"
	"github.com/knadh/koanf/providers/env/v2"
	"github.com/knadh/koanf/providers/file"
	"github.com/knadh/koanf/v2"
	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/store/shared/db"
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
		Agent    Agent    `koanf:"agent"`
		Database Database `koanf:"database"`
		HTTP     HTTP     `koanf:"http"`
		GitHub   GitHub   `koanf:"github"`
		Labels   []Label  `koanf:"labels"`
		Logger   Logger   `koanf:"logger"`
		Metric   Metric   `koanf:"metric"`
		Reaper   Reaper   `koanf:"reaper"`
		RPC      RPC      `koanf:"rpc"`
		Server   Server   `koanf:"server"`
		Session  Session  `koanf:"session"`
		Users    []User   `koanf:"users"`
	}

	// Agent provides the agent configuration.
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

	// Database provides the database configuration.
	Database struct {
		Driver         db.Driver `koanf:"driver"`
		Datasource     string    `koanf:"datasource"`
		Secret         string    `koanf:"secret"`
		MaxConnections int       `koanf:"max_connections"`
	}

	// GitHub provides the GitHub integration configuration.
	GitHub struct {
		Server    string    `koanf:"server"`
		APIServer string    `koanf:"api_server"`
		App       GitHubApp `koanf:"app"`
		OAuth     OAuth     `koanf:"oauth"`
	}

	// GitHubApp provides the GitHub App configuration.
	GitHubApp struct {
		IntegrationID int64  `koanf:"integration_id"`
		WebhookSecret string `koanf:"webhook_secret"`
		PrivateKey    string `koanf:"private_key"`
	}

	// OAuth provides the OAuth application configuration.
	OAuth struct {
		ClientID     string `koanf:"client_id"`
		ClientSecret string `koanf:"client_secret"`
	}

	// HTTP provides http security configuration.
	HTTP struct {
		AllowedHosts          []string          `koand:"allowed_hosts"`
		HostsProxyHeaders     []string          `koand:"hosts_proxy_headers"`
		SSLRedirect           bool              `koand:"ssl_redirect"`
		SSLTemporaryRedirect  bool              `koand:"ssl_temporary_redirect"`
		SSLHost               string            `koand:"ssl_host"`
		SSLProxyHeaders       map[string]string `koand:"ssl_proxy_headers"`
		STSSeconds            int64             `koand:"sts_seconds"`
		STSIncludeSubdomains  bool              `koand:"sts_include_subdomains"`
		STSPreload            bool              `koand:"sts_preload"`
		ForceSTSHeader        bool              `koand:"force_sts_header"`
		BrowserXSSFilter      bool              `koand:"browser_xss_filter"`
		FrameDeny             bool              `koand:"frame_deny"`
		ContentTypeNosniff    bool              `koand:"content_type_nosniff"`
		ContentSecurityPolicy string            `koand:"content_security_policy"`
		ReferrerPolicy        string            `koand:"referrer_policy"`
	}

	// Metric provides the metrics configuration.
	Metric struct {
		Secret string `koanf:"secret"`
	}

	// Reaper provides the reaper configuration.
	Reaper struct {
		Disabled bool          `koanf:"disabled"`
		Interval time.Duration `koanf:"interval"`
		Reclaim  time.Duration `koanf:"reclaim"`
	}

	// RPC provides the RPC server configuration.
	RPC struct {
		Host   string `koanf:"host"`
		Proto  string `koanf:"proto"`
		Secret string `koanf:"secret"`
	}

	// Server provides the server server configuration.
	Server struct {
		Acme  bool   `koanf:"acme"`
		Addr  string `koanf:"addr"`
		Cert  string `koanf:"cert"`
		Email string `koanf:"email"`
		Key   string `koanf:"key"`
		Host  string `koanf:"host"`
		Port  int    `koanf:"port"`
		Debug bool   `koanf:"debug"`
	}

	// Session provides the session configuration.
	Session struct {
		Secret  string        `koanf:"secret"`
		Timeout time.Duration `koanf:"timeout"`
		Secure  bool          `koanf:"secure"`
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

	// User stores account information used to bootstrap admin user account(s)
	// on system initialization.
	User struct {
		Admin bool   `koanf:"admin"`
		Login string `koanf:"login"`
		Token string `koanf:"token"`
	}
)

// Load loads the configuration from a file and environment variable.
func Load(path string) (*Config, error) {
	k := koanf.New(".")

	// Load configuration from specified TOML file first
	err := k.Load(file.Provider(path), toml.Parser())
	if err != nil {
		return nil, fmt.Errorf("config: failed to load configuration file at '%s': %w", path, err)
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
		return nil, fmt.Errorf("config: failed to read configuration file: %w", err)
	}

	if config.Agent.Name == "" {
		config.Agent.Name = hostname
	}

	return &config, nil
}
