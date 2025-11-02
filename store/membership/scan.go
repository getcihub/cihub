package membership

import (
	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
)

// toParams converts the Membership structure to a set of named query parameters.
func toParams(m *core.Membership) map[string]interface{} {
	return map[string]interface{}{
		"membership_installation_id": m.InstallationID,
		"membership_user_id":         m.UserID,
		"membership_role":            m.Role,
		"membership_state":           m.State,
		"membership_created":         m.Created,
		"membership_synced":          m.Synced,
		"membership_updated":         m.Updated,
	}
}

// scanRow scans the sql.Row and copies the column values to the destination object.
func scanRow(scanner db.Scanner, dest *core.Membership) error {
	return scanner.Scan(
		&dest.InstallationID,
		&dest.UserID,
		&dest.Role,
		&dest.State,
		&dest.Created,
		&dest.Synced,
		&dest.Updated,
	)
}
