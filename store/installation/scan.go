package installation

import (
	"database/sql"

	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
)

// ToParams converts the Installation structure to a set of named query parameters.
func ToParams(i *core.Installation) map[string]interface{} {
	return map[string]interface{}{
		"installation_id":        i.ID,
		"installation_login":     i.Login,
		"installation_avatar":    i.Avatar,
		"installation_type":      i.Type,
		"installation_created":   i.Created,
		"installation_suspended": i.Suspended,
		"installation_updated":   i.Updated,
	}
}

// scanRow scans the sql.Row and copies the column values to the destination object.
func scanRow(scanner db.Scanner, dest *core.Installation) error {
	return scanner.Scan(
		&dest.ID,
		&dest.Login,
		&dest.Avatar,
		&dest.Type,
		&dest.Created,
		&dest.Suspended,
		&dest.Updated,
	)
}

// scanRows scans multiple rows and copies the column values to a slice of Installation objects.
func scanRows(rows *sql.Rows) ([]*core.Installation, error) {
	defer func() {
		if err := rows.Close(); err != nil {
			logrus.WithError(err).
				Warnln("store: cannot close installation rows")
		}
	}()

	installations := []*core.Installation{}
	for rows.Next() {
		installation := new(core.Installation)
		err := scanRow(rows, installation)
		if err != nil {
			return nil, err
		}
		installations = append(installations, installation)
	}
	return installations, nil
}
