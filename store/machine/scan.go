package machine

import (
	"database/sql"
	"strings"

	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
)

// toParams converts the Machine structure to a set of named query parameters.
func toParams(m *core.Machine) map[string]interface{} {
	var labels string
	if len(m.Labels) > 0 {
		labels = strings.Join(m.Labels, ",")
	}

	return map[string]interface{}{
		"machine_name":          m.Name,
		"machine_owner":         m.Owner,
		"machine_arch":          m.Arch,
		"machine_cpu":           m.CPU,
		"machine_cpu_limit":     m.CPULimit,
		"machine_cpu_allocated": m.CPUAllocated,
		"machine_ram_total":     m.RAMTotal,
		"machine_ram_available": m.RAMAvailable,
		"machine_ram_limit":     m.RAMLimit,
		"machine_ram_allocated": m.RAMAllocated,
		"machine_status":        m.Status,
		"machine_created":       m.Created,
		"machine_last_seen":     m.LastSeen,
		"machine_updated":       m.Updated,
		"machine_token":         m.Token,
		"machine_labels":        labels,
	}
}

// scanRow scans a sql.Row and copies the column values to the destination object.
func scanRow(scanner db.Scanner, dest *core.Machine) error {
	var labels string
	err := scanner.Scan(
		&dest.Name,
		&dest.Owner,
		&dest.Arch,
		&dest.CPU,
		&dest.CPULimit,
		&dest.CPUAllocated,
		&dest.RAMTotal,
		&dest.RAMAvailable,
		&dest.RAMLimit,
		&dest.RAMAllocated,
		&dest.Status,
		&dest.Created,
		&dest.LastSeen,
		&dest.Updated,
		&dest.Token,
		&labels,
	)

	if err != nil {
		return err
	}

	if labels != "" {
		dest.Labels = strings.Split(labels, ",")
	}

	return nil
}

// scanRows scans sql.Rows from a machines query.
func scanRows(rows *sql.Rows) ([]*core.Machine, error) {
	defer func() {
		if err := rows.Close(); err != nil {
			logrus.WithError(err).
				Warnln("store: cannot close machine rows")
		}
	}()

	machines := []*core.Machine{}
	for rows.Next() {
		machine := new(core.Machine)
		err := scanRow(rows, machine)
		if err != nil {
			return nil, err
		}
		machines = append(machines, machine)
	}

	return machines, nil
}
