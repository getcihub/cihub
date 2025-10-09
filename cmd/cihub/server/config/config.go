package config

import (
	"fmt"
	"time"

	"github.com/getcihub/cihub/store/shared/db"
	"github.com/knadh/koanf/parsers/toml"
	"github.com/knadh/koanf/providers/file"
	"github.com/knadh/koanf/v2"
	"github.com/sirupsen/logrus"
)

type (
	// Config provides the system configuration.
	Config struct {
		Database Database `koanf:"database"`
		HTTP     HTTP     `koanf:"http"`
		Labels   []Label  `koanf:"labels"`
		Logger   Logger   `koanf:"logger"`
		Metric   Metric   `koanf:"metric"`
		RPC      RPC      `koanf:"rpc"`
		Server   Server   `koanf:"server"`
		Session  Session  `koanf:"session"`
		Users    []User   `koanf:"users"`
	}

	// Database provides the database configuration.
	Database struct {
		Driver         db.Driver `koanf:"driver"`
		Datasource     string    `koanf:"datasource"`
		Secret         string    `koanf:"secret"`
		MaxConnections int       `koanf:"max_connections"`
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
		Admin   bool   `koanf:"admin"`
		Login   string `koanf:"login"`
		Machine bool   `koanf:"machine"`
		Token   string `koanf:"token"`
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

	var config Config
	if err := k.Unmarshal("", &config); err != nil {
		return nil, fmt.Errorf("config: failed to read configuration file: %w", err)
	}

	return &config, nil
}
