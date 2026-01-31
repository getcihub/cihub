package core

// System stores system information.
type System struct {
	AppName        string `json:"app_name"`
	InstallationID int64  `json:"installation_id"`
	Server         string `json:"server"`
}
