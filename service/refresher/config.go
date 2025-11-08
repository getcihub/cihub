package refresher

// Config provides the refresher configuration.
type Config struct {
	ClientID     string
	ClientSecret string
	Endpoint     string
}

// NewConfig returns a new refresher configuration.
func NewConfig(clientID, clientSecret, endpoint string) Config {
	return Config{
		ClientID:     clientID,
		ClientSecret: clientSecret,
		Endpoint:     endpoint,
	}
}
